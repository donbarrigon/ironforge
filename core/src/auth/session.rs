//! Sesiones de usuarios con login: RAM (shards) + persistencia en MongoDB.
//!
//! Diseñado para un solo proceso (sin balanceador): todas las instancias del
//! servidor comparten este `Session` global.
//!
//! - Cookie = base64url(token ‖ HMAC-SHA256(app.key, token)). Un token inventado
//!   se rechaza sin consultar Mongo.
//! - En Mongo solo se guarda `token_hash` = SHA-256(token).
//! - El usuario vive en RAM como `Arc<dyn Any>` y se obtiene con downcast a `T`.
//! - `last_activity`/`expires_at` se actualizan con atómicos (solo lock de lectura)
//!   y se escriben a Mongo por lote cada `sync_interval_seconds`.
//! - Al superar `max_in_memory` se expulsan de RAM las más inactivas (siguen en Mongo).

use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use ahash::AHashMap;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use mongodb::bson::spec::BinarySubtype;
use mongodb::bson::{Binary, DateTime, Document, doc, oid::ObjectId};
use mongodb::options::IndexOptions;
use mongodb::{Collection, Database, IndexModel};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::config::{SameSite, env};
use crate::db::mongo_odm::odm;
use crate::{Auth, Authenticatable, ForgeError, log};

/// Token de sesión: 32 bytes aleatorios de un CSPRNG.
pub type Token = [u8; 32];

type HmacSha256 = Hmac<Sha256>;

const MAC_LEN: usize = 32;
/// Máximo de updates por comando en la sincronización por lote.
const SYNC_CHUNK: usize = 1000;

// === Entrada en RAM =============================================

struct Entry {
    /// `_id` del documento en Mongo.
    id: ObjectId,
    user_id: ObjectId,
    user: Arc<dyn Any + Send + Sync>,
    created_at: i64,
    remember: bool,
    last_activity: AtomicI64,
    expires_at: AtomicI64,
    /// Hay cambios de actividad sin escribir a Mongo.
    dirty: AtomicBool,
}

enum Lookup<T: Authenticatable> {
    Hit(Auth<T>),
    Expired,
    Miss,
    /// La sesión existe pero guarda otro tipo de usuario: error de programación,
    /// no se borra la sesión.
    Mismatch,
}

// === Session ====================================================

pub struct Session {
    shards: Box<[RwLock<AHashMap<Token, Entry>>]>,
    mask: usize,
    len: AtomicUsize,
    /// user_id → tokens de sus sesiones en RAM.
    by_user: Mutex<AHashMap<ObjectId, Vec<Token>>>,
    /// Actividad de sesiones expulsadas con cambios pendientes: `_id → (last_activity, expires_at)`.
    pending: Mutex<AHashMap<ObjectId, (i64, i64)>>,
    /// Single-flight: una sola consulta a Mongo por token a la vez.
    loading: Mutex<AHashMap<Token, Arc<tokio::sync::Mutex<()>>>>,
    evicting: AtomicBool,
    key: Vec<u8>,
    db: Database,
    coll: Collection<Document>,
}

static SESSION: OnceLock<Session> = OnceLock::new();

/// Instancia global. Requiere `Session::init()`.
pub fn session() -> Result<&'static Session, ForgeError> {
    SESSION
        .get()
        .ok_or_else(|| ForgeError::internal().message("Session not initialized"))
}

impl Session {
    // === Inicio / apagado =======================================

    /// Crea los índices, inicializa el singleton y lanza las tareas de fondo.
    /// Requiere `ODMongo::init()` antes.
    pub async fn init() -> Result<(), ForgeError> {
        let cfg = &env().session;
        let key = decode_app_key(&env().app.key).map_err(|e| ForgeError::internal().message(e))?;
        let db = odm()?.db();
        let coll = db.collection::<Document>(&cfg.collection);

        let indexes = [
            IndexModel::builder()
                .keys(doc! { "token_hash": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            IndexModel::builder()
                .keys(doc! { "expires_at": 1 })
                .options(IndexOptions::builder().expire_after(Duration::ZERO).build())
                .build(),
            IndexModel::builder().keys(doc! { "user_id": 1 }).build(),
        ];
        coll.create_indexes(indexes)
            .await
            .map_err(|e| db_err("session::init", "Failed to create session indexes", e))?;

        let n = cfg.shard_count();
        let shards = (0..n).map(|_| RwLock::new(AHashMap::new())).collect();

        let store = Session {
            shards,
            mask: n - 1,
            len: AtomicUsize::new(0),
            by_user: Mutex::new(AHashMap::new()),
            pending: Mutex::new(AHashMap::new()),
            loading: Mutex::new(AHashMap::new()),
            evicting: AtomicBool::new(false),
            key,
            db,
            coll,
        };
        SESSION
            .set(store)
            .map_err(|_| ForgeError::internal().message("Session already initialized"))?;

        spawn_tasks();
        log::info("Session initialized", None);
        Ok(())
    }

    /// Escribe a Mongo la actividad pendiente. Se llama al apagar el servidor.
    pub async fn flush(&self) {
        self.sync().await;
    }

    // === API pública ============================================

    /// Login: crea la sesión con token nuevo (Mongo directo). La cookie se arma con
    /// `set_cookie_header(&auth.session_id, auth.expires_at)`.
    pub async fn create<T: Authenticatable>(&self, user: T, remember: bool) -> Result<Auth<T>, ForgeError> {
        let token = new_token()?;
        let now = now_ms();
        let id = ObjectId::new();
        let user_id = user.auth_id();
        let expires_at = compute_expires(now, now, remember);
        let user_bson = mongodb::bson::to_bson(&user).map_err(|e| {
            let msg = "Failed to serialize session user";
            log::error(msg, Some(json!({ "mod": "session::create", "error": e.to_string() })));
            ForgeError::internal().message(msg).caused_by(e)
        })?;

        self.coll
            .insert_one(doc! {
                "_id": id,
                "token_hash": hash_token(&token),
                "user_id": user_id,
                "user": user_bson,
                "created_at": DateTime::from_millis(now),
                "last_activity": DateTime::from_millis(now),
                "expires_at": DateTime::from_millis(expires_at),
                "remember": remember,
            })
            .await
            .map_err(|e| db_err("session::create", "Failed to create session", e))?;

        let user = Arc::new(user);
        self.insert(
            token,
            Entry {
                id,
                user_id,
                user: Arc::clone(&user) as Arc<dyn Any + Send + Sync>,
                created_at: now,
                remember,
                last_activity: AtomicI64::new(now),
                expires_at: AtomicI64::new(expires_at),
                dirty: AtomicBool::new(false),
            },
        );
        Ok(Auth {
            session_id: token,
            user_id,
            user,
            created_at: now,
            last_activity: now,
            expires_at,
            remember,
            ip: None,
            user_agent: None,
        })
    }

    /// Busca en RAM (o en Mongo) y renueva la actividad: `last_activity` y `expires_at`.
    /// `None` si la sesión no existe, venció o guarda otro tipo de usuario.
    pub async fn get<T: Authenticatable>(&self, token: &Token) -> Option<Auth<T>> {
        match self.touch::<T>(token, now_ms()) {
            Lookup::Hit(auth) => Some(auth),
            Lookup::Expired => {
                self.remove_token(token);
                None
            }
            Lookup::Mismatch => None,
            Lookup::Miss => self.load::<T>(*token).await,
        }
    }

    /// Token nuevo para la misma sesión (cambio de privilegios). Invalida el token viejo.
    pub async fn regenerate(&self, old: &Token) -> Result<Token, ForgeError> {
        let new = new_token()?;

        let res = self
            .coll
            .update_one(
                doc! { "token_hash": hash_token(old), "expires_at": { "$gt": DateTime::now() } },
                doc! { "$set": { "token_hash": hash_token(&new) } },
            )
            .await
            .map_err(|e| db_err("session::regenerate", "Failed to regenerate session", e))?;

        let entry = self.remove_token(old);
        if res.matched_count == 0 {
            return Err(ForgeError::unauthorized());
        }
        if let Some(entry) = entry {
            self.insert(new, entry);
        }
        Ok(new)
    }

    /// Logout: borra la sesión de RAM y de Mongo.
    pub async fn destroy(&self, token: &Token) -> Result<(), ForgeError> {
        self.remove_token(token);
        self.coll
            .delete_one(doc! { "token_hash": hash_token(token) })
            .await
            .map_err(|e| db_err("session::destroy", "Failed to destroy session", e))?;
        Ok(())
    }

    /// Cierra todas las sesiones del usuario (cambio de contraseña, bloqueo).
    pub async fn invalidate_user(&self, user_id: ObjectId) -> Result<(), ForgeError> {
        let tokens = self.by_user.lock().unwrap().remove(&user_id).unwrap_or_default();
        for token in tokens {
            if self.shard(&token).write().unwrap().remove(&token).is_some() {
                self.len.fetch_sub(1, Ordering::Relaxed);
            }
        }
        self.coll
            .delete_many(doc! { "user_id": user_id })
            .await
            .map_err(|e| db_err("session::invalidate_user", "Failed to invalidate user sessions", e))?;
        Ok(())
    }

    /// Reemplaza la copia del usuario en todas sus sesiones (RAM y Mongo).
    pub async fn refresh_user<T: Authenticatable>(&self, user: T) -> Result<(), ForgeError> {
        let user_id = user.auth_id();
        let user_bson = mongodb::bson::to_bson(&user).map_err(|e| {
            let msg = "Failed to serialize session user";
            log::error(
                msg,
                Some(json!({ "mod": "session::refresh_user", "error": e.to_string() })),
            );
            ForgeError::internal().message(msg).caused_by(e)
        })?;

        let user: Arc<dyn Any + Send + Sync> = Arc::new(user);
        let tokens = self.by_user.lock().unwrap().get(&user_id).cloned().unwrap_or_default();
        for token in tokens {
            if let Some(entry) = self.shard(&token).write().unwrap().get_mut(&token) {
                entry.user = Arc::clone(&user);
            }
        }

        self.coll
            .update_many(doc! { "user_id": user_id }, doc! { "$set": { "user": user_bson } })
            .await
            .map_err(|e| db_err("session::refresh_user", "Failed to refresh session user", e))?;
        Ok(())
    }

    /// `Set-Cookie` firmado con la configuración de `env.session`. `Max-Age` es lo que
    /// le queda a la sesión, así la cookie vence junto con la sesión del servidor.
    pub fn set_cookie_header(&self, token: &Token, expires_at: i64) -> String {
        let max_age = (expires_at - now_ms()).max(0) / 1000;
        let mut s = format!(
            "{}={}; Path=/; Max-Age={max_age}",
            env().session.cookie_name,
            self.encode_cookie(token)
        );
        push_cookie_attrs(&mut s);
        s
    }

    /// `Set-Cookie` que borra la cookie de sesión (logout).
    pub fn clear_cookie_header(&self) -> String {
        let mut s = format!("{}=; Path=/; Max-Age=0", env().session.cookie_name);
        push_cookie_attrs(&mut s);
        s
    }

    // === RAM ====================================================

    fn shard(&self, token: &Token) -> &RwLock<AHashMap<Token, Entry>> {
        // El token es aleatorio y va firmado: sus bytes sirven directo como hash.
        let h = u64::from_le_bytes(token[..8].try_into().unwrap()) as usize;
        &self.shards[h & self.mask]
    }

    /// Busca en RAM y renueva la actividad con solo lock de lectura.
    fn touch<T: Authenticatable>(&self, token: &Token, now: i64) -> Lookup<T> {
        let shard = self.shard(token).read().unwrap();
        let Some(entry) = shard.get(token) else {
            return Lookup::Miss;
        };
        if entry.expires_at.load(Ordering::Relaxed) <= now {
            return Lookup::Expired;
        }

        let Ok(user) = Arc::clone(&entry.user).downcast::<T>() else {
            log::error(
                "Session user type mismatch",
                Some(json!({ "mod": "session::get", "expected": std::any::type_name::<T>() })),
            );
            return Lookup::Mismatch;
        };

        entry.last_activity.fetch_max(now, Ordering::Relaxed);
        let expires_at = compute_expires(entry.created_at, now, entry.remember);
        entry.expires_at.fetch_max(expires_at, Ordering::Relaxed);
        entry.dirty.store(true, Ordering::Relaxed);

        Lookup::Hit(Auth {
            session_id: *token,
            user_id: entry.user_id,
            user,
            created_at: entry.created_at,
            last_activity: entry.last_activity.load(Ordering::Relaxed),
            expires_at: entry.expires_at.load(Ordering::Relaxed),
            remember: entry.remember,
            ip: None,
            user_agent: None,
        })
    }

    fn insert(&self, token: Token, entry: Entry) {
        let user_id = entry.user_id;
        let prev = self.shard(&token).write().unwrap().insert(token, entry);
        if prev.is_none() {
            self.len.fetch_add(1, Ordering::Relaxed);
            self.by_user.lock().unwrap().entry(user_id).or_default().push(token);
        }
        self.maybe_evict();
    }

    fn remove_token(&self, token: &Token) -> Option<Entry> {
        let entry = self.shard(token).write().unwrap().remove(token)?;
        self.len.fetch_sub(1, Ordering::Relaxed);
        self.unindex(entry.user_id, token);
        Some(entry)
    }

    fn unindex(&self, user_id: ObjectId, token: &Token) {
        let mut by_user = self.by_user.lock().unwrap();
        if let Some(tokens) = by_user.get_mut(&user_id) {
            tokens.retain(|t| t != token);
            if tokens.is_empty() {
                by_user.remove(&user_id);
            }
        }
    }

    /// Al superar `max_in_memory`, suelta de RAM las más inactivas hasta `evict_target()`.
    /// Las que tienen cambios pendientes pasan a `pending` para la próxima sincronización.
    fn maybe_evict(&self) {
        let cfg = &env().session;
        if self.len.load(Ordering::Relaxed) <= cfg.max_in_memory || self.evicting.swap(true, Ordering::Acquire) {
            return;
        }

        let mut all: Vec<(i64, Token)> = Vec::with_capacity(self.len.load(Ordering::Relaxed));
        for shard in self.shards.iter() {
            let shard = shard.read().unwrap();
            all.extend(shard.iter().map(|(t, e)| (e.last_activity.load(Ordering::Relaxed), *t)));
        }

        let excess = all.len().saturating_sub(cfg.evict_target());
        if excess > 0 {
            all.select_nth_unstable_by_key(excess - 1, |(last, _)| *last);
            for (_, token) in &all[..excess] {
                if let Some(e) = self.remove_token(token) {
                    if e.dirty.load(Ordering::Relaxed) {
                        let activity = (e.last_activity.into_inner(), e.expires_at.into_inner());
                        self.pending.lock().unwrap().insert(e.id, activity);
                    }
                }
            }
        }
        self.evicting.store(false, Ordering::Release);
    }

    // === Mongo ==================================================

    /// Carga desde Mongo con single-flight: si varias requests piden el mismo token,
    /// solo una consulta y las demás lo encuentran en RAM.
    async fn load<T: Authenticatable>(&self, token: Token) -> Option<Auth<T>> {
        let gate = Arc::clone(self.loading.lock().unwrap().entry(token).or_default());
        let guard = gate.lock().await;

        let auth = match self.touch::<T>(&token, now_ms()) {
            Lookup::Hit(auth) => Some(auth),
            Lookup::Expired => {
                self.remove_token(&token);
                None
            }
            Lookup::Mismatch => None,
            Lookup::Miss => self.load_from_db::<T>(token).await,
        };

        drop(guard);
        let mut loading = self.loading.lock().unwrap();
        if loading.get(&token).is_some_and(|g| Arc::ptr_eq(g, &gate)) {
            loading.remove(&token);
        }
        auth
    }

    async fn load_from_db<T: Authenticatable>(&self, token: Token) -> Option<Auth<T>> {
        let doc = self
            .coll
            .find_one(doc! { "token_hash": hash_token(&token) })
            .await
            .map_err(|e| db_err("session::load", "Failed to load session", e))
            .ok()??;

        let id = doc.get_object_id("_id").ok()?;
        let user_id = doc.get_object_id("user_id").ok()?;
        let created_at = doc.get_datetime("created_at").ok()?.timestamp_millis();
        let remember = doc.get_bool("remember").unwrap_or(false);
        let mut expires_at = doc.get_datetime("expires_at").ok()?.timestamp_millis();

        // Si fue expulsada con actividad sin sincronizar, esa actividad es más reciente.
        if let Some((_, pending_exp)) = self.pending.lock().unwrap().remove(&id) {
            expires_at = expires_at.max(pending_exp);
        }

        // El TTL de Mongo corre cada ~60 s: puede devolver documentos ya vencidos.
        let now = now_ms();
        if expires_at <= now {
            return None;
        }

        let user: T = doc
            .get("user")
            .cloned()
            .map(mongodb::bson::from_bson)?
            .map_err(|e| {
                log::error(
                    "Failed to deserialize session user",
                    Some(json!({ "mod": "session::load", "error": e.to_string() })),
                );
            })
            .ok()?;
        let user = Arc::new(user);
        let expires_at = expires_at.max(compute_expires(created_at, now, remember));

        self.insert(
            token,
            Entry {
                id,
                user_id,
                user: Arc::clone(&user) as Arc<dyn Any + Send + Sync>,
                created_at,
                remember,
                last_activity: AtomicI64::new(now),
                expires_at: AtomicI64::new(expires_at),
                dirty: AtomicBool::new(true),
            },
        );

        Some(Auth {
            session_id: token,
            user_id,
            user,
            created_at,
            last_activity: now,
            expires_at,
            remember,
            ip: None,
            user_agent: None,
        })
    }

    /// Escribe a Mongo, por lote, la actividad de las entradas sucias y de las expulsadas.
    /// `$max` evita pisar un valor más reciente.
    async fn sync(&self) {
        let mut updates: Vec<Document> = Vec::new();
        let update = |id: ObjectId, last: i64, exp: i64| {
            doc! {
                "q": { "_id": id },
                "u": { "$max": {
                    "last_activity": DateTime::from_millis(last),
                    "expires_at": DateTime::from_millis(exp),
                } },
            }
        };

        for shard in self.shards.iter() {
            let shard = shard.read().unwrap();
            for entry in shard.values() {
                if entry.dirty.swap(false, Ordering::Relaxed) {
                    updates.push(update(
                        entry.id,
                        entry.last_activity.load(Ordering::Relaxed),
                        entry.expires_at.load(Ordering::Relaxed),
                    ));
                }
            }
        }
        let pending = std::mem::take(&mut *self.pending.lock().unwrap());
        updates.extend(pending.into_iter().map(|(id, (last, exp))| update(id, last, exp)));

        for chunk in updates.chunks(SYNC_CHUNK) {
            let cmd = doc! { "update": self.coll.name(), "updates": chunk.to_vec(), "ordered": false };
            if let Err(e) = self.db.run_command(cmd).await {
                log::error(
                    "Failed to sync sessions",
                    Some(json!({ "mod": "session::sync", "error": e.to_string(), "count": chunk.len() })),
                );
            }
        }
    }

    /// Quita de RAM las vencidas. En Mongo las borra el índice TTL.
    fn purge(&self) {
        let now = now_ms();
        let mut removed: Vec<(ObjectId, Token)> = Vec::new();
        for shard in self.shards.iter() {
            let mut shard = shard.write().unwrap();
            shard.retain(|token, e| {
                let alive = e.expires_at.load(Ordering::Relaxed) > now;
                if !alive {
                    removed.push((e.user_id, *token));
                }
                alive
            });
        }
        self.len.fetch_sub(removed.len(), Ordering::Relaxed);
        for (user_id, token) in &removed {
            self.unindex(*user_id, token);
        }
    }

    // === Cookie =================================================

    fn mac(&self, token: &Token) -> HmacSha256 {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC acepta claves de cualquier largo");
        mac.update(token);
        mac
    }

    fn encode_cookie(&self, token: &Token) -> String {
        let mut raw = [0u8; 32 + MAC_LEN];
        raw[..32].copy_from_slice(token);
        raw[32..].copy_from_slice(&self.mac(token).finalize().into_bytes());
        URL_SAFE_NO_PAD.encode(raw)
    }

    /// Verifica la firma (tiempo constante) y devuelve el token.
    pub fn decode_cookie(&self, cookie: &str) -> Option<Token> {
        let raw = URL_SAFE_NO_PAD.decode(cookie.trim()).ok()?;
        if raw.len() != 32 + MAC_LEN {
            return None;
        }
        let token: Token = raw[..32].try_into().ok()?;
        self.mac(&token).verify_slice(&raw[32..]).ok()?;
        Some(token)
    }
}

// === Tareas de fondo ============================================

fn spawn_tasks() {
    let cfg = &env().session;
    let purge_every = cfg.purge_interval().max(Duration::from_secs(1));
    let sync_every = cfg.sync_interval().max(Duration::from_secs(1));

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(purge_every);
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Some(s) = SESSION.get() {
                s.purge();
            }
        }
    });

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(sync_every);
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Some(s) = SESSION.get() {
                s.sync().await;
            }
        }
    });
}

// === Helpers ====================================================

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Expiración deslizante. Con "recordarme": `remember_days` desde la última actividad.
/// Sin él: `idle_timeout`, acotado por `absolute_timeout` desde la creación.
fn compute_expires(created_at: i64, last_activity: i64, remember: bool) -> i64 {
    let cfg = &env().session;
    if remember {
        return last_activity + cfg.remember().as_millis() as i64;
    }
    let idle = last_activity + cfg.idle_timeout().as_millis() as i64;
    match cfg.absolute_timeout() {
        Some(abs) => idle.min(created_at + abs.as_millis() as i64),
        None => idle,
    }
}

fn new_token() -> Result<Token, ForgeError> {
    let mut token = [0u8; 32];
    getrandom::fill(&mut token).map_err(|e| {
        let msg = "Failed to generate session token";
        log::error(
            msg,
            Some(json!({ "mod": "session::new_token", "error": e.to_string() })),
        );
        ForgeError::internal().message(msg)
    })?;
    Ok(token)
}

fn hash_token(token: &Token) -> Binary {
    Binary {
        subtype: BinarySubtype::Generic,
        bytes: Sha256::digest(token).to_vec(),
    }
}

/// `base64:...` se decodifica; cualquier otro valor se usa tal cual como bytes.
fn decode_app_key(key: &str) -> Result<Vec<u8>, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("app.key is empty".into());
    }
    match key.strip_prefix("base64:") {
        Some(b64) => STANDARD
            .decode(b64)
            .map_err(|e| format!("app.key is not valid base64: {e}")),
        None => Ok(key.as_bytes().to_vec()),
    }
}

fn push_cookie_attrs(s: &mut String) {
    let cfg = &env().session;
    if let Some(domain) = &cfg.cookie_domain {
        s.push_str(&format!("; Domain={domain}"));
    }
    if cfg.cookie_secure {
        s.push_str("; Secure");
    }
    if cfg.cookie_http_only {
        s.push_str("; HttpOnly");
    }
    s.push_str(match cfg.cookie_same_site {
        SameSite::Strict => "; SameSite=Strict",
        SameSite::Lax => "; SameSite=Lax",
        SameSite::None => "; SameSite=None",
    });
}

fn db_err(module: &str, msg: &str, e: mongodb::error::Error) -> ForgeError {
    log::error(msg, Some(json!({ "mod": module, "error": e.to_string() })));
    ForgeError::internal().message(msg.to_string()).caused_by(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac_cookie(key: &[u8], token: &Token) -> String {
        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(token);
        let mut raw = token.to_vec();
        raw.extend_from_slice(&mac.finalize().into_bytes());
        URL_SAFE_NO_PAD.encode(raw)
    }

    fn verify(key: &[u8], cookie: &str) -> Option<Token> {
        let raw = URL_SAFE_NO_PAD.decode(cookie).ok()?;
        if raw.len() != 32 + MAC_LEN {
            return None;
        }
        let token: Token = raw[..32].try_into().ok()?;
        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(&token);
        mac.verify_slice(&raw[32..]).ok()?;
        Some(token)
    }

    #[test]
    fn cookie_firmada_valida_y_rechaza_alteraciones() {
        let key = decode_app_key(crate::config::DEFAULT_APP_KEY).unwrap();
        assert_eq!(key.len(), 32);
        let token = new_token().unwrap();
        let cookie = mac_cookie(&key, &token);
        assert_eq!(verify(&key, &cookie), Some(token));

        // Otra clave no valida la firma.
        assert_eq!(verify(b"otra-clave", &cookie), None);

        // Token alterado.
        let mut raw = URL_SAFE_NO_PAD.decode(&cookie).unwrap();
        raw[0] ^= 1;
        assert_eq!(verify(&key, &URL_SAFE_NO_PAD.encode(raw)), None);

        // Basura.
        assert_eq!(verify(&key, "no-es-una-cookie"), None);
    }

    #[test]
    fn tokens_distintos_y_hash_de_32_bytes() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_ne!(a, b);
        assert_eq!(hash_token(&a).bytes.len(), 32);
    }

    #[test]
    fn app_key_invalida() {
        assert!(decode_app_key("").is_err());
        assert!(decode_app_key("base64:%%%").is_err());
        assert_eq!(decode_app_key("texto").unwrap(), b"texto");
    }
}
