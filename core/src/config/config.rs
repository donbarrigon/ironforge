use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Env {
    pub app: AppEnv,
    pub db: DbEnv,
    pub server: Vec<ServerEnv>,
    pub log: LogEnv,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEnv {
    pub name: String,
    pub debug: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbEnv {
    pub name: String,
    pub string_connection: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEnv {
    pub print: bool,
    pub level: String,
    pub days: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEnv {
    pub host: String,
    pub port: u16,
    pub https: bool,
    pub auto_cert: bool,
}

// singleton global
static ENV: OnceLock<Env> = OnceLock::new();

pub fn env() -> &'static Env {
    return ENV
        .get()
        .expect("ENV is not initialized — call load_env() before accessing the environment");
}

pub fn load_env() -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string("config/env.json")?;
    let config: Env = serde_json::from_str(&content)?;
    ENV.set(config)
        .map_err(|_| "ENV has already been initialized — load_env() can only be called once".into())
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    load_env()?;
    crate::log::init();
    return Ok(());
}
