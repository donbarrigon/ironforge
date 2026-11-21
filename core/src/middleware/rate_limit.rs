//! Límite de requests en memoria (un solo proceso, igual que `Session`).
//!
//! Tres sabores, según contra qué se cuenta:
//! - [`rate_limit`]: por IP **y ruta**. Cada ruta del grupo tiene su propia cuenta.
//! - [`rate_limit_ip`]: solo por IP. Todas las rutas del grupo comparten la cuenta.
//! - [`rate_limit_group`]: por IP dentro de un grupo con nombre. Todos los
//!   `rate_limit_group(.., "x")` comparten la cuenta, estén donde estén en el router.
//!
//! Comunes a los tres:
//! - Ventana fija: empieza con el primer request y dura `per_seconds`. Al llegar
//!   a `max` responde 429 con `Retry-After` hasta que la ventana vence.
//! - `rate_limit` y `rate_limit_ip` tienen un contador propio por llamada: dos
//!   `rate_limit_ip(5, 60)` no comparten cuenta (para eso está `rate_limit_group`).
//! - IPv6 se cuenta por /64: un cliente suele tener el prefijo entero y podría
//!   rotar direcciones para saltarse el límite.
//! - Las ventanas vencidas se purgan al pasar por su shard (sin tareas de fondo).

use std::future::{Ready, ready};
use std::hash::Hash;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ahash::{AHashMap, RandomState};
use hyper::header;

use crate::server::dispatch::WILDCARD_PARAM;
use crate::server::router::Param;
use crate::{Context, ForgeError};

const SHARDS: usize = 16;

/// Limita a `max` requests cada `per_seconds` por IP **y ruta**.
///
/// La ruta es el path del request (lo que va antes del `?`), sin los tramos
/// variables: `/sessions/{id}` cuenta igual con cualquier `id`.
///
/// ```ignore
/// r.group("/auth", rate_limit(5, 60), |r| {
///     r.post("/login", auth::login);       // 5 por minuto
///     r.post("/register", auth::register); // otros 5 por minuto
/// });
/// ```
pub fn rate_limit(
    max: u32,
    per_seconds: u64,
) -> impl Fn(&mut Context) -> Ready<Result<(), ForgeError>> + Clone + Send + Sync + 'static {
    let limiter = Arc::new(RateLimiter::new(max, Duration::from_secs(per_seconds)));
    move |c: &mut Context| {
        // Sin IP no hay a quién contar (no pasa con conexiones TCP normales).
        let Some(ip) = c.ip() else {
            return ready(Ok(()));
        };
        let key = (client_key(ip), route_key(c.r.uri().path(), &c.params));
        ready(limiter.handle(c, key))
    }
}

/// Limita a `max` requests cada `per_seconds` por IP, sumando todas las rutas
/// donde se use este middleware.
///
/// ```ignore
/// r.group("/auth", rate_limit_ip(5, 60), |r| {
///     r.post("/login", auth::login);       // entre las dos,
///     r.post("/register", auth::register); // 5 por minuto
/// });
/// ```
pub fn rate_limit_ip(
    max: u32,
    per_seconds: u64,
) -> impl Fn(&mut Context) -> Ready<Result<(), ForgeError>> + Clone + Send + Sync + 'static {
    let limiter = Arc::new(RateLimiter::new(max, Duration::from_secs(per_seconds)));
    move |c: &mut Context| handle_ip(&limiter, c)
}

/// Limita a `max` requests cada `per_seconds` por IP dentro del grupo `group`.
/// Todas las llamadas con el mismo `group` comparten la cuenta.
///
/// Usar el mismo `group` con otro `max` o `per_seconds` es un error de
/// configuración y entra en pánico al armar el router.
///
/// ```ignore
/// r.group("/users/profile", rate_limit_group(30, 60, "users"), |r| { ... });
/// r.group("/users/auth", rate_limit_group(30, 60, "users"), |r| { ... });
/// ```
pub fn rate_limit_group(
    max: u32,
    per_seconds: u64,
    group: &str,
) -> impl Fn(&mut Context) -> Ready<Result<(), ForgeError>> + Clone + Send + Sync + 'static {
    let limiter = group_limiter(max, per_seconds, group);
    move |c: &mut Context| handle_ip(&limiter, c)
}

fn handle_ip(limiter: &RateLimiter<IpAddr>, c: &mut Context) -> Ready<Result<(), ForgeError>> {
    // Sin IP no hay a quién contar (no pasa con conexiones TCP normales).
    let Some(ip) = c.ip() else {
        return ready(Ok(()));
    };
    ready(limiter.handle(c, client_key(ip)))
}

/// Limiter compartido del grupo; lo crea la primera vez que se pide.
fn group_limiter(max: u32, per_seconds: u64, group: &str) -> Arc<RateLimiter<IpAddr>> {
    static GROUPS: OnceLock<Mutex<AHashMap<String, Arc<RateLimiter<IpAddr>>>>> = OnceLock::new();
    let window = Duration::from_secs(per_seconds);
    let mut groups = GROUPS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let limiter = groups
        .entry(group.to_string())
        .or_insert_with(|| Arc::new(RateLimiter::new(max, window)));
    assert!(
        limiter.max == max && limiter.window_ms == window.as_millis() as i64,
        "rate_limit_group: el grupo `{group}` ya existe con otro max o per_seconds"
    );
    limiter.clone()
}

// === Limiter ====================================================

struct RateLimiter<K> {
    max: u32,
    window_ms: i64,
    hasher: RandomState,
    shards: Box<[Mutex<Shard<K>>]>,
}

struct Shard<K> {
    windows: AHashMap<K, Window>,
    next_purge_ms: i64,
}

impl<K> Default for Shard<K> {
    fn default() -> Self {
        Self { windows: AHashMap::new(), next_purge_ms: 0 }
    }
}

struct Window {
    count: u32,
    ends_at_ms: i64,
}

impl<K: Hash + Eq> RateLimiter<K> {
    fn new(max: u32, window: Duration) -> Self {
        assert!(max > 0, "rate_limit: max debe ser mayor que 0");
        assert!(!window.is_zero(), "rate_limit: per_seconds debe ser mayor que 0");
        Self {
            max,
            window_ms: window.as_millis() as i64,
            hasher: RandomState::new(),
            shards: (0..SHARDS).map(|_| Mutex::new(Shard::default())).collect(),
        }
    }

    fn handle(&self, c: &mut Context, key: K) -> Result<(), ForgeError> {
        let Err(retry_after_ms) = self.hit(key, now_ms()) else {
            return Ok(());
        };
        let seconds = (retry_after_ms + 999) / 1000;
        c.set_header(header::RETRY_AFTER, &seconds.to_string())?;
        Err(ForgeError::too_many_requests().message(format!("Too many requests, try again in {seconds} seconds")))
    }

    /// Cuenta un request. `Err(ms)` = bloqueado, con los ms que faltan para que se libere.
    fn hit(&self, key: K, now: i64) -> Result<(), i64> {
        let i = self.hasher.hash_one(&key) as usize % SHARDS;
        // Nunca hay `.await` con el lock tomado; si un hilo entró en pánico, el mapa sigue usable.
        let mut shard = self.shards[i].lock().unwrap_or_else(|e| e.into_inner());

        if now >= shard.next_purge_ms {
            shard.windows.retain(|_, w| w.ends_at_ms > now);
            shard.next_purge_ms = now + self.window_ms;
        }

        let w = shard.windows.entry(key).or_insert(Window { count: 0, ends_at_ms: 0 });
        if w.ends_at_ms <= now {
            w.count = 0;
            w.ends_at_ms = now + self.window_ms;
        }
        if w.count >= self.max {
            return Err(w.ends_at_ms - now);
        }
        w.count += 1;
        Ok(())
    }
}

/// IPv4 tal cual; IPv6 recortada a su /64 (IPv4 mapeada a IPv6 se trata como IPv4).
fn client_key(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & (u128::MAX << 64))),
        v4 => v4,
    }
}

/// Ruta del request sin los tramos variables, en minúsculas (el router compara
/// los tramos fijos sin mayúsculas). `path` es el de la URI, ya sin query.
///
/// Los tramos se parten igual que en `dispatch::resolve`, así el resto del `/*`
/// ocupa exactamente los últimos tramos. Los `{param}` se buscan de atrás hacia
/// adelante: lo normal es que los tramos fijos vayan antes de las variables.
fn route_key(path: &str, params: &[Param]) -> Box<str> {
    let path = path.trim_start_matches('/');
    let mut parts: Vec<&str> = if path.is_empty() { Vec::new() } else { path.split('/').collect() };

    if let Some(rest) = params.iter().find(|p| p.name == WILDCARD_PARAM).filter(|p| !p.value.is_empty()) {
        let n = rest.value.split('/').count();
        parts.truncate(parts.len().saturating_sub(n));
    }

    let mut vars = params.iter().rev().filter(|p| p.name != WILDCARD_PARAM).map(|p| p.value.as_str()).peekable();
    let mut kept: Vec<&str> = Vec::with_capacity(parts.len());
    for part in parts.into_iter().rev() {
        if vars.peek() == Some(&part) {
            vars.next();
        } else if !part.is_empty() {
            kept.push(part);
        }
    }

    let mut key = String::with_capacity(path.len() + 1);
    for part in kept.iter().rev() {
        key.push('/');
        key.push_str(part);
    }
    if key.is_empty() {
        key.push('/');
    }
    key.make_ascii_lowercase();
    key.into_boxed_str()
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const NOW: i64 = 1_800_000_000_000;
    const A: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    const B: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

    fn limiter<K: Hash + Eq>(max: u32, seconds: u64) -> RateLimiter<K> {
        RateLimiter::new(max, Duration::from_secs(seconds))
    }

    fn param(name: &str, value: &str) -> Param {
        Param { name: name.into(), value: value.into() }
    }

    #[test]
    fn bloquea_al_pasar_el_maximo() {
        let l = limiter(3, 60);
        for _ in 0..3 {
            assert_eq!(l.hit(A, NOW), Ok(()));
        }
        assert_eq!(l.hit(A, NOW + 10_000), Err(50_000));
    }

    #[test]
    fn se_libera_al_vencer_la_ventana() {
        let l = limiter(1, 60);
        assert_eq!(l.hit(A, NOW), Ok(()));
        assert!(l.hit(A, NOW + 59_999).is_err());
        assert_eq!(l.hit(A, NOW + 60_000), Ok(()));
    }

    #[test]
    fn cada_ip_cuenta_aparte() {
        let l = limiter(1, 60);
        assert_eq!(l.hit(A, NOW), Ok(()));
        assert_eq!(l.hit(B, NOW), Ok(()));
        assert!(l.hit(A, NOW).is_err());
    }

    #[test]
    fn cada_limiter_cuenta_aparte() {
        let (l1, l2) = (limiter(1, 60), limiter(1, 60));
        assert_eq!(l1.hit(A, NOW), Ok(()));
        assert_eq!(l2.hit(A, NOW), Ok(()));
    }

    #[test]
    fn ipv6_por_prefijo_64() {
        let l = limiter(1, 60);
        let a: IpAddr = "2001:db8:1:2::1".parse().unwrap();
        let mismo_64: IpAddr = "2001:db8:1:2:ffff::9".parse().unwrap();
        let otro_64: IpAddr = "2001:db8:1:3::1".parse().unwrap();
        assert_eq!(l.hit(client_key(a), NOW), Ok(()));
        assert!(l.hit(client_key(mismo_64), NOW).is_err());
        assert_eq!(l.hit(client_key(otro_64), NOW), Ok(()));
    }

    #[test]
    fn ipv4_mapeada_cuenta_como_ipv4() {
        let l = limiter(1, 60);
        assert_eq!(l.hit(client_key(A), NOW), Ok(()));
        assert!(l.hit(client_key("::ffff:10.0.0.1".parse().unwrap()), NOW).is_err());
    }

    #[test]
    fn purga_las_vencidas() {
        let l = limiter(5, 60);
        for i in 0..100u8 {
            let _ = l.hit(IpAddr::V4(Ipv4Addr::new(10, 1, 0, i)), NOW);
        }
        // Un request tras vencer todas purga su shard; repetir en todos los shards.
        for i in 0..100u8 {
            let _ = l.hit(IpAddr::V4(Ipv4Addr::new(10, 2, 0, i)), NOW + 120_000);
        }
        let quedan_viejas: usize = l
            .shards
            .iter()
            .map(|s| s.lock().unwrap().windows.values().filter(|w| w.ends_at_ms <= NOW + 120_000).count())
            .sum();
        assert_eq!(quedan_viejas, 0);
    }

    // === rate_limit: por ruta ===================================

    #[test]
    fn cada_ruta_cuenta_aparte() {
        let l = limiter(1, 60);
        let login = route_key("/api/auth/login", &[]);
        let register = route_key("/api/auth/register", &[]);
        assert_eq!(l.hit((A, login.clone()), NOW), Ok(()));
        assert_eq!(l.hit((A, register), NOW), Ok(()));
        assert!(l.hit((A, login), NOW).is_err());
    }

    #[test]
    fn ruta_estatica() {
        assert_eq!(&*route_key("/api/users/me", &[]), "/api/users/me");
        assert_eq!(&*route_key("/", &[]), "/");
        assert_eq!(&*route_key("", &[]), "/");
    }

    #[test]
    fn ruta_sin_mayusculas_ni_barras_sobrantes() {
        assert_eq!(&*route_key("/API/Users/ME/", &[]), "/api/users/me");
    }

    #[test]
    fn ruta_dinamica_ignora_la_variable() {
        let k1 = route_key("/api/sessions/abc", &[param("id", "abc")]);
        let k2 = route_key("/api/sessions/XYZ", &[param("id", "XYZ")]);
        assert_eq!(&*k1, "/api/sessions");
        assert_eq!(k1, k2);
    }

    #[test]
    fn ruta_con_varias_variables() {
        let k = route_key("/api/posts/7/comments/9", &[param("post", "7"), param("comment", "9")]);
        assert_eq!(&*k, "/api/posts/comments");
    }

    #[test]
    fn variable_igual_a_un_tramo_fijo() {
        // `/api/users/{id}` con id = "users": se quita el último, no el fijo.
        let k = route_key("/api/users/users", &[param("id", "users")]);
        assert_eq!(&*k, "/api/users");
    }

    #[test]
    fn ruta_wildcard_ignora_el_resto() {
        let k = route_key("/resources/js/auth.js", &[param(WILDCARD_PARAM, "js/auth.js")]);
        assert_eq!(&*k, "/resources");
        let vacio = route_key("/resources", &[param(WILDCARD_PARAM, "")]);
        assert_eq!(&*vacio, "/resources");
    }

    // === rate_limit_group =======================================

    #[test]
    fn mismo_grupo_comparte_la_cuenta() {
        let g1 = group_limiter(1, 60, "test_comparte");
        let g2 = group_limiter(1, 60, "test_comparte");
        assert_eq!(g1.hit(A, NOW), Ok(()));
        assert!(g2.hit(A, NOW).is_err());
    }

    #[test]
    fn grupos_distintos_cuentan_aparte() {
        let g1 = group_limiter(1, 60, "test_aparte_1");
        let g2 = group_limiter(1, 60, "test_aparte_2");
        assert_eq!(g1.hit(A, NOW), Ok(()));
        assert_eq!(g2.hit(A, NOW), Ok(()));
    }

    #[test]
    #[should_panic]
    fn mismo_grupo_con_otra_config_es_error() {
        group_limiter(1, 60, "test_config");
        group_limiter(2, 60, "test_config");
    }

    /// Solo tiene que compilar: el router acepta los tres middlewares en grupos.
    #[allow(dead_code)]
    fn se_puede_usar_en_el_router(r: &mut crate::server::RouterBuilder) {
        r.middleware(rate_limit(5, 60), |_| {});
        r.group("/api", rate_limit_ip(60, 60), |_| {});
        r.group("/users", rate_limit_group(30, 60, "users"), |_| {});
    }

    #[test]
    #[should_panic]
    fn max_cero_es_error_de_configuracion() {
        limiter::<IpAddr>(0, 60);
    }
}
