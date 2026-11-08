# Contexto: diseño de `core/src/auth/session_store.rs`

Resumen de una conversación previa de diseño. Lee también `CLAUDE.md`, `core/src/auth/auth.rs`, `core/src/handler/context.rs`, `core/src/config/config.rs` y `core/src/cache/` (código comentado de un cache anterior con `AHashMap` + msgpack).

## Objetivo

Un `SessionStore` que mantiene sesiones en RAM con persistencia en MongoDB. Al llegar al límite de RAM expulsa las más inactivas (quedan en Mongo y se recargan al volver a usarse). Purga periódica de sesiones vencidas. Todo configurable en `env.json`.

## Decisiones ya tomadas

**Límite por número de sesiones**, no por bytes (evita estimar memoria).

**Persistencia en MongoDB** (no archivos). Colección `sessions`. El usuario autenticado se guarda en el documento con formato libre (Mongo es flexible) y `user_id` indexado para facilitar eliminaciones. La carga extra sobre Mongo se considera mínima.

**Estructura en RAM:**
- Índice principal: `AHashMap<SessionKey, Entry>` partido en shards (`RwLock<AHashMap>` por shard; shard elegido con `hash & (n - 1)`, n potencia de 2).
- **Sin BTreeMap secundario**: `last_activity` (y `expires_at` con expiración deslizante) cambian en cada request; mantener un índice ordenado obligaría a escribir en cada lectura.
- `last_activity` y `expires_at` como `AtomicI64` en la entrada → actualizarlos solo requiere lock de lectura.
- Purga de vencidas en RAM: barrido por shard con `retain`, cada `purge_interval_seconds`.
- Expulsión por lote: al superar `max_in_memory`, juntar `(last_activity, key)`, `select_nth_unstable` para el bloque más antiguo, bajar hasta `evict_target()` (90%).
- Expulsar = soltar de RAM (ya está en Mongo); si la entrada tiene cambios pendientes, sincronizar primero.
- Índice `user_id → sesiones` (`AHashMap<ObjectId, SmallVec<[SessionKey; 4]>>`), necesario para invalidar/refrescar por usuario.

Boceto:
```rust
struct Entry {
    user_id: ObjectId,
    data: /* ver decisión pendiente 1 */,
    expires_at: AtomicI64,
    last_activity: AtomicI64,
}
struct SessionStore {
    shards: Box<[RwLock<AHashMap<SessionKey, Entry>>]>,
}
```

**Token de sesión:**
- **No usar ObjectId como token** (es predecible: timestamp + contador). Usar 32 bytes aleatorios de un CSPRNG (`getrandom` / `OsRng`), clave tipo `[u8; 32]` (hoy `Auth.session_id` y `Context.session_id` son `String`; cambiarlos).
- En Mongo guardar **solo el hash SHA-256 del token** (`token_hash`), con `_id: ObjectId` normal.
- `ahash::RandomState` usa semilla aleatoria → sin riesgo de HashDoS aunque la clave venga de la cookie.

**Sincronización con Mongo:**
- Login (crear) y logout (borrar) van directo a Mongo en el momento.
- `last_activity` y demás cambios: marcar entrada como sucia y cada `sync_interval_seconds` hacer un solo `bulk_write`. Usar `$max` en `last_activity` para que un nodo con valor viejo no pise uno más reciente.

**Índices de `sessions`:**
```js
db.sessions.createIndex({ token_hash: 1 }, { unique: true })
db.sessions.createIndex({ expires_at: 1 }, { expireAfterSeconds: 0 }) // TTL
db.sessions.createIndex({ user_id: 1 })
```
`expires_at` debe guardarse como **BSON DateTime** (no `i64`), si no el TTL lo ignora. En el struct se puede mantener `i64` (ms) y convertir con `bson::DateTime::from_millis`.

## Configuración (ya escrita)

`core/src/config/config.rs` ahora tiene `SessionEnv` (en `Env.session`), enum `SameSite` y defaults en todos los structs. Helpers: `idle_timeout()`, `absolute_timeout()`, `remember()`, `purge_interval()`, `sync_interval()`, `evict_target()`, `shard_count()`.

Defaults (pensados para 4 cores / 8 GB, Mongo posiblemente en la misma máquina):

| Campo | Valor |
|---|---|
| max_in_memory | 100000 (~1–2 KB por sesión → 100–200 MB) |
| evict_to_percent | 90 |
| shards | 16 |
| idle_timeout_minutes | 120 (expiración deslizante) |
| absolute_timeout_hours | 24 (0 = sin límite) |
| remember_days | 30 |
| purge_interval_seconds | 60 |
| collection | "sessions" |
| sync_interval_seconds | 30 |
| cookie_name / secure / http_only / same_site / domain | "forge_session" / true / true / "lax" / null |

`env.json` se actualizó: `server` pasó de arreglo a objeto (como espera `server.rs`), se agregó `app.locale: "es"` y la sección `session`. Falta exportar `SessionEnv` y `SameSite` en `config/mod.rs` si se usan fuera del módulo.

## Decisiones confirmadas (2026-11-08)

- **(1)** El usuario se guarda en RAM como `Arc<dyn Any + Send + Sync>` y se hace downcast a `T` al leer.
- **(4)** Solo hay sesión para usuarios con login; `Auth` existe solo para sesiones iniciadas. Nada de sesiones de invitado.
- **(3)** Un solo proceso, sin balanceador. Las instancias del cluster viven en el mismo proceso Rust y comparten el mismo `SessionStore` en RAM, así que no hay que sincronizar nada entre nodos. Si algún día hacen falta varios nodos, se hará un store distinto para eso; este no lo contempla.
- **Nombre**: el struct se llama `Session` (no `SessionStore`), en `core/src/auth/session.rs`.
- **API pública aprobada**:
```rust
impl Session {
    /// Login: crea la sesión (Mongo directo) con token nuevo; devuelve el valor de la cookie (token + HMAC).
    pub async fn create<T: Authenticatable>(&self, user: T, remember: bool) -> Result<String, ForgeError>;
    /// Middleware: verifica el HMAC, busca en RAM (o en Mongo con single-flight), renueva la actividad.
    pub async fn get<T: Authenticatable>(&self, cookie: &str) -> Option<Arc<T>>;
    /// Cambio de privilegios en una sesión abierta: token nuevo, misma sesión; devuelve la cookie nueva.
    pub async fn regenerate(&self, cookie: &str) -> Result<String, ForgeError>;
    /// Logout: borra de RAM y de Mongo.
    pub async fn destroy(&self, cookie: &str) -> Result<(), ForgeError>;
    /// Cambio de contraseña o bloqueo: cierra todas las sesiones del usuario.
    pub async fn invalidate_user(&self, user_id: ObjectId) -> Result<(), ForgeError>;
    /// Cambio de roles o datos: reemplaza la copia del usuario en sus sesiones activas.
    pub fn refresh_user<T: Authenticatable>(&self, user_id: ObjectId, user: T);
}
```
  Tareas de fondo (`tokio::spawn`, lanzadas en `init()`): purga de vencidas, expulsión y `bulk_write` periódico a Mongo; al apagar, sincronizar lo pendiente.
- **Filosofía**: diseño específico para el caso real, no genérico para todos los casos. Más simple y más óptimo.

## Decisiones PENDIENTES (resolver antes de codificar)

1. **Qué guardar en RAM, dado que `Auth<T: Authenticatable>` es genérico** y el store global (`OnceLock`) no puede serlo. Recomendado: el usuario deserializado como `Arc<dyn Any + Send + Sync>` con downcast a `T` al leer (cero deserialización por request). Alternativas: bytes msgpack deserializados por request, o solo `user_id` + carga desde Mongo (anula el propósito de RAM).
2. **Permisos desactualizados**: al guardar copia del usuario, hacen falta `invalidate_user(user_id)` (cambio de contraseña, bloqueo) y `refresh_user(user_id, user)` (cambio de roles).
3. **Varios nodos (`core/src/cluster/`)**: cada proceso tiene su RAM; un logout en un nodo no se ve en otro. Opciones: sticky sessions, change streams de Mongo (requiere replica set) o difundir invalidaciones por el cluster propio. Recomendado: dejar en el store un punto de entrada claro para invalidaciones externas y decidir el mecanismo después.
4. **¿Sesiones para invitados?** Recomendado: crear sesión solo al hacer login (evita que bots llenen RAM y Mongo). CSRF/flash para invitados, en cookie firmada aparte.

## Otras consideraciones

- **Fijación de sesión**: generar token nuevo al hacer login y al cambiar privilegios.
- **Proteger a Mongo de tokens falsos**: cookie = `token + HMAC(app_key, token)` para rechazar tokens inventados sin consultar la base. `app.key` ya existe en `AppEnv` / `env.json` (default `DEFAULT_APP_KEY` solo válido con `debug: true`).
- **Carga concurrente (single-flight)**: si varias requests piden una sesión que no está en RAM, solo una consulta Mongo.
- **Verificar `expires_at` al cargar desde Mongo**: el TTL corre ~cada 60 s y puede devolver documentos ya vencidos.
- **Tareas de fondo** con `tokio::spawn` + intervalo: copiar datos bajo el lock, soltarlo y después hablar con Mongo.
- **Apagado ordenado**: en SIGTERM sincronizar las sesiones sucias.
- **Integración con `Context`**: middleware que lee la cookie, resuelve la sesión y deja `Auth<T>` en `extensions`.
- **Tests**: inyectar reloj (`now_ms`) y backend de persistencia como traits para probar expiración/expulsión sin Mongo ni tiempo real.

## Estado (2026-11-08)

`Session` implementado en `core/src/auth/session.rs`. Diferencias con la API aprobada:
- `get` devuelve `Option<Auth<T>>` (incluye metadatos de la sesión), no `Option<Arc<T>>`.
- `refresh_user(user)` es `async` y devuelve `Result` (también actualiza Mongo); el `user_id` sale de `user.auth_id()`.
- Extras: `Session::init()` (tras `ODMongo::init()`), `session()`, `flush()` (lo llama `Server::listen` al apagar), `set_cookie_header` / `clear_cookie_header`.
- Sincronización por lote con el comando `update` (varios statements, `ordered: false`) en vez de `bulk_write` del cliente (que exige MongoDB 8).

## Siguiente paso

Middleware que lea la cookie, llame a `session().get::<T>()` y deje `Auth<T>` en `Context.extensions` (y `Context.session_id`).
