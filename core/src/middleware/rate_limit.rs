//! Límite de requests por IP, en memoria (un solo proceso, igual que `Session`).
//!
//! - Ventana fija por IP: empieza con su primer request y dura `per_seconds`.
//!   Al llegar a `max` responde 429 con `Retry-After` hasta que la ventana vence.
//! - Cada `rate_limit(...)` tiene su propio contador: un grupo de rutas con
//!   `rate_limit(5, 60)` no comparte cuenta con otro que use `rate_limit(60, 60)`.
//! - IPv6 se cuenta por /64: un cliente suele tener el prefijo entero y podría
//!   rotar direcciones para saltarse el límite.
//! - Las ventanas vencidas se purgan al pasar por su shard (sin tareas de fondo).

use std::future::{Ready, ready};
use std::net::{IpAddr, Ipv6Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ahash::{AHashMap, RandomState};
use hyper::header;

use crate::{Context, ForgeError};

const SHARDS: usize = 16;

/// Middleware que limita a `max` requests por IP cada `per_seconds`.
///
/// ```ignore
/// r.middleware(rate_limit(5, 60), |r| {
///     r.post("/login", auth::login);
/// });
/// ```
pub fn rate_limit(
    max: u32,
    per_seconds: u64,
) -> impl Fn(&mut Context) -> Ready<Result<(), ForgeError>> + Clone + Send + Sync + 'static {
    let limiter = Arc::new(RateLimiter::new(max, Duration::from_secs(per_seconds)));
    move |c: &mut Context| ready(limiter.handle(c))
}

// === Limiter ====================================================

struct RateLimiter {
    max: u32,
    window_ms: i64,
    hasher: RandomState,
    shards: Box<[Mutex<Shard>]>,
}

#[derive(Default)]
struct Shard {
    windows: AHashMap<IpAddr, Window>,
    next_purge_ms: i64,
}

struct Window {
    count: u32,
    ends_at_ms: i64,
}

impl RateLimiter {
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

    fn handle(&self, c: &mut Context) -> Result<(), ForgeError> {
        // Sin IP no hay a quién contar (no pasa con conexiones TCP normales).
        let Some(ip) = c.ip() else {
            return Ok(());
        };
        let Err(retry_after_ms) = self.hit(ip, now_ms()) else {
            return Ok(());
        };
        let seconds = (retry_after_ms + 999) / 1000;
        c.set_header(header::RETRY_AFTER, &seconds.to_string())?;
        Err(ForgeError::too_many_requests().message(format!("Too many requests, try again in {seconds} seconds")))
    }

    /// Cuenta un request. `Err(ms)` = bloqueado, con los ms que faltan para que se libere.
    fn hit(&self, ip: IpAddr, now: i64) -> Result<(), i64> {
        let key = client_key(ip);
        let i = self.hasher.hash_one(key) as usize % SHARDS;
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

    fn limiter(max: u32, seconds: u64) -> RateLimiter {
        RateLimiter::new(max, Duration::from_secs(seconds))
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
        assert_eq!(l.hit(a, NOW), Ok(()));
        assert!(l.hit(mismo_64, NOW).is_err());
        assert_eq!(l.hit(otro_64, NOW), Ok(()));
    }

    #[test]
    fn ipv4_mapeada_cuenta_como_ipv4() {
        let l = limiter(1, 60);
        assert_eq!(l.hit(A, NOW), Ok(()));
        assert!(l.hit("::ffff:10.0.0.1".parse().unwrap(), NOW).is_err());
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

    /// Solo tiene que compilar: el router acepta el middleware en grupos.
    #[allow(dead_code)]
    fn se_puede_usar_en_el_router(r: &mut crate::server::RouterBuilder) {
        r.middleware(rate_limit(5, 60), |_| {});
        r.group("/api", rate_limit(60, 60), |_| {});
    }

    #[test]
    #[should_panic]
    fn max_cero_es_error_de_configuracion() {
        limiter(0, 60);
    }
}
