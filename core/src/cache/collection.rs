// use ahash::AHashMap;
// use bytes::Bytes;
// use serde::{Deserialize, Serialize};
// use std::time::{Duration, SystemTime, UNIX_EPOCH};

// use crate::{
//     ForgeError,
//     cache::config::{CacheMode, ConfigCollection, ExpiryState, PersistState},
// };

// pub(crate) const NEVER: u64 = 0;

// #[derive(Serialize, Deserialize)]
// pub struct Collection {
//     name: String,
//     mode: CacheMode,
//     data: AHashMap<String, Entry>,
//     expiry: ExpiryState,
//     persist_state: PersistState,
//     default_ttl: u64, // in miliseconds, 0 = forever
// }

// #[derive(Serialize, Deserialize)]
// pub struct Entry {
//     value: Bytes,    // msgpack ya serializado
//     expires_at: u64, // ms Unix, 0 = forever
// }

// impl Entry {
//     pub fn new<T: Serialize + ?Sized>(value: &T, ttl: u64) -> Self {
//         Self {
//             value: bincode::serialize(value).unwrap(),
//             expires_at: now_ms() + ttl,
//         }
//     }
// }

// impl Collection {
//     pub fn new(name: impl Into<String>, config: Option<ConfigCollection>) -> Self {
//         Self::with_config(name, config.unwrap_or_else(ConfigCollection::default))
//     }
//     fn with_config(name: impl Into<String>, config: ConfigCollection) -> Self {
//         Self {
//             name: name.into(),
//             mode: config.mode,
//             data: AHashMap::new(),
//             expiry: match config.expiry_interval {
//                 Some(x) => ExpiryState::new(x.as_millis() as u64),
//                 None => ExpiryState::default(),
//             },
//             persist_state: match config.persist_interval {
//                 Some(x) => PersistState::new(x.as_millis() as u64),
//                 None => PersistState::default(),
//             },
//         }
//     }

//     pub fn push<T: Serialize + ?Sized>(&mut self, key: impl Into<String>, value: &T) -> Result<(), ForgeError> {
//         self.data.insert(key.into(), Entry::new(value, self.default_ttl));
//         Ok(())
//     }

//     fn now_ms() -> u64 {
//         SystemTime::now()
//             .duration_since(UNIX_EPOCH)
//             .map(|d| d.as_millis() as u64)
//             .unwrap_or(0)
//     }
// }
