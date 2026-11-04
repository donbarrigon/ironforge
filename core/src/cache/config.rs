use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct ConfigCollection {
    pub mode: CacheMode,
    pub expiry_interval: Option<Duration>,
    pub persist_interval: Option<Duration>,
}

impl Default for ConfigCollection {
    fn default() -> Self {
        ConfigCollection {
            mode: CacheMode::Memory,
            expiry_interval: Some(Duration::from_secs(30)),
            persist_interval: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum CacheMode {
    Memory, // data de cache solo en memoria
    Disk,   // data de cache en disco
    Mixed,  // data de cache en memoria y disco
}

#[derive(Serialize, Deserialize)]
pub struct ExpiryState {
    interval: u64,           // in miliseconds
    last_purge_at: u64,      // in miliseconds last time purged
    last_purge_disk_at: u64, // in miliseconds last time purged in dick
    next_purge_at: u64,      // in miliseconds next time to purge
}

impl Default for ExpiryState {
    fn default() -> Self {
        ExpiryState {
            interval: 30000, // 30 second for default, 0 = forever
            last_purge_at: 0,
            last_purge_disk_at: 0,
            next_purge_at: 0,
        }
    }
}

impl ExpiryState {
    pub fn new(interval: u64) -> Self {
        ExpiryState {
            interval,
            last_purge_at: 0,
            last_purge_disk_at: 0,
            next_purge_at: 0,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PersistState {
    interval: u64,        // in miliseconds
    last_persist_at: u64, // in miliseconds last time persisted
    dirty: bool,
}

impl Default for PersistState {
    fn default() -> Self {
        PersistState {
            interval: 0,        // 0 = forever
            last_persist_at: 0, // last time persisted
            dirty: false,       // have changes from last persist
        }
    }
}

impl PersistState {
    pub fn new(interval: u64) -> Self {
        PersistState {
            interval,
            last_persist_at: 0,
            dirty: false,
        }
    }
}
