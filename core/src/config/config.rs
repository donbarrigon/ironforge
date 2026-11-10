use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

use crate::Session;
use crate::db::mongo_odm::ODMongo;

// === Env ========================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Env {
    pub app: AppEnv,
    pub db: DbEnv,
    pub server: ServerEnv,
    pub log: LogEnv,
    pub session: SessionEnv,
}

// === App ========================================================

/// Clave por defecto, válida (32 bytes en base64) pero pública y siempre igual.
/// Solo para desarrollo: en producción definir `app.key` en env.json.
pub const DEFAULT_APP_KEY: &str = "base64:MseNkLzOQVPMWqLoejyLWQsR2GEScY6knf5QygwlQGs=";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppEnv {
    pub name: String,
    pub debug: bool,
    pub url: String,
    pub locale: String,
    /// Clave secreta de la app (32 bytes en base64, prefijo `base64:`).
    /// Se usa para firmar/cifrar cookies, tokens, etc.
    /// Por defecto usa `DEFAULT_APP_KEY` (estática): cambiarla en producción.
    pub key: String,
}

impl Default for AppEnv {
    fn default() -> Self {
        Self {
            name: "forgeApp".into(),
            debug: false,
            url: "https://localhost:8443".into(),
            locale: "es".into(),
            key: DEFAULT_APP_KEY.into(),
        }
    }
}

impl AppEnv {
    /// `DEFAULT_APP_KEY` solo se permite en desarrollo (`debug: true`).
    /// En producción es un error: la clave es pública y permitiría firmar tokens.
    pub fn validate_key(&self) -> Result<(), String> {
        if self.debug {
            return Ok(());
        }
        if self.key.trim().is_empty() {
            return Err("app.key está vacía: defínela en env.json".into());
        }
        if self.key == DEFAULT_APP_KEY {
            return Err("app.key usa la clave por defecto (solo desarrollo): genera una propia en env.json".into());
        }
        Ok(())
    }
}

// === Db =========================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DbEnv {
    pub name: String,
    pub string_connection: String,
}

impl Default for DbEnv {
    fn default() -> Self {
        Self {
            name: "sample_mflix".into(),
            string_connection: "mongodb://localhost:27017".into(),
        }
    }
}

// === Log ========================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LogEnv {
    pub print: bool,
    pub level: String,
    pub days: u8,
}

impl Default for LogEnv {
    fn default() -> Self {
        Self {
            print: true,
            level: "info".into(),
            days: 7,
        }
    }
}

// === Server =====================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerEnv {
    pub host: String,
    pub port: u16,
    pub https: bool,
    pub auto_cert: bool,
}

impl Default for ServerEnv {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8443,
            https: true,
            auto_cert: true,
        }
    }
}

// === Session ====================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionEnv {
    // --- Memoria ---
    /// Máximo de sesiones en RAM. Al superarlo se expulsan las más inactivas
    /// (quedan solo en Mongo y se recargan si vuelven a usarse).
    pub max_in_memory: usize,
    /// Al expulsar, baja hasta este % de max_in_memory (evita expulsar en cada request).
    pub evict_to_percent: u8,
    /// Número de shards del mapa. Potencia de 2.
    pub shards: usize,

    // --- Expiración ---
    /// Se cierra tras este tiempo sin actividad (expiración deslizante).
    pub idle_timeout_minutes: u64,
    /// Duración máxima aunque haya actividad. 0 = sin límite absoluto.
    pub absolute_timeout_hours: u64,
    /// Duración de las sesiones con "recordarme".
    pub remember_days: u64,
    /// Cada cuánto se purgan las vencidas de RAM.
    pub purge_interval_seconds: u64,

    // --- Persistencia (Mongo) ---
    pub collection: String,
    /// Cada cuánto se escriben a Mongo los cambios pendientes (last_activity, etc.).
    pub sync_interval_seconds: u64,

    // --- Cookie ---
    pub cookie_name: String,
    pub cookie_secure: bool,
    pub cookie_http_only: bool,
    pub cookie_same_site: SameSite,
    pub cookie_domain: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

impl Default for SessionEnv {
    fn default() -> Self {
        Self {
            max_in_memory: 100_000,
            evict_to_percent: 90,
            shards: 16,

            idle_timeout_minutes: 120,
            absolute_timeout_hours: 24,
            remember_days: 30,
            purge_interval_seconds: 60,

            collection: "sessions".into(),
            sync_interval_seconds: 30,

            cookie_name: "forge_session".into(),
            cookie_secure: true,
            cookie_http_only: true,
            cookie_same_site: SameSite::Lax,
            cookie_domain: None,
        }
    }
}

impl SessionEnv {
    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.idle_timeout_minutes * 60)
    }

    pub fn absolute_timeout(&self) -> Option<Duration> {
        (self.absolute_timeout_hours > 0).then(|| Duration::from_secs(self.absolute_timeout_hours * 3600))
    }

    pub fn remember(&self) -> Duration {
        Duration::from_secs(self.remember_days * 86_400)
    }

    pub fn purge_interval(&self) -> Duration {
        Duration::from_secs(self.purge_interval_seconds)
    }

    pub fn sync_interval(&self) -> Duration {
        Duration::from_secs(self.sync_interval_seconds)
    }

    /// Número de sesiones al que se baja tras expulsar.
    pub fn evict_target(&self) -> usize {
        self.max_in_memory * self.evict_to_percent.min(99) as usize / 100
    }

    /// Shards ajustados a potencia de 2 (mínimo 1), para elegir con `hash & (n - 1)`.
    pub fn shard_count(&self) -> usize {
        self.shards.max(1).next_power_of_two()
    }
}

// === Singleton global ===========================================

static ENV: OnceLock<Env> = OnceLock::new();

pub fn env() -> &'static Env {
    return ENV
        .get()
        .expect("ENV is not initialized — call load_env() before accessing the environment");
}

pub fn load_env() -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string("env.json")?;
    let config: Env = serde_json::from_str(&content)?;
    config.app.validate_key()?;
    ENV.set(config)
        .map_err(|_| "ENV has already been initialized — load_env() can only be called once".into())
}

pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
    load_env()?;
    crate::log::init(); // inicializa el logger
    ODMongo::init().await?;
    Session::init().await?;
    crate::handler::headers::init(); // inicializa los bytes de fallback
    return Ok(());
}
