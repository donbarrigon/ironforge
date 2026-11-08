# ironforge

Framework web en Rust inspirado en Laravel. Monorepo (workspace de Cargo, edición 2024):

- `core/` → crate `ironforge`: servidor (hyper + tokio + rustls), router/dispatch, cluster, ODM de MongoDB, validadores, auth, cache, i18n, log, config.
- `macros/` → crate `ironforge-macros`: macros procedurales (syn 2, quote, darling).

Proyectos relacionados (otros repos): `ironforge_new` (starter kit) y `blacksmith` (CLI).

## Convenciones

- **Base de datos: solo MongoDB por ahora.** PostgreSQL vendrá después como feature. IDs y referencias son `mongodb::bson::oid::ObjectId`.
- Comentarios y mensajes en español.
- Configuración global en `core/src/config/config.rs`: `Env` cargado desde `env.json` en un `OnceLock`, accesible con `env()`. Todos los structs de config tienen `Default` y `#[serde(default)]`. Los campos de tiempo llevan la unidad en el nombre (`_seconds`, `_minutes`, `_hours`, `_days`).
- Singletons globales con `OnceLock` (ver `ENV` y `ODMONGO`).
- Errores con `ForgeError` (`core/src/error/`).
- Formato con `rustfmt.toml` del repo.
- Nunca hacer `.await` con un lock tomado; usar `std::sync::RwLock`/`Mutex` para estado en memoria.

## Comandos

- `cargo check` / `cargo build` / `cargo test` desde la raíz del workspace.
