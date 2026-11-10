use bytes::Bytes;
use serde_json::json;
use std::io::ErrorKind;
use std::path::PathBuf;

use crate::ForgeError;
use crate::log;

/// Carpeta raíz del storage local, relativa al directorio donde corre la app.
const ROOT: &str = "public/storage";

// ─── Lv ─────────────────────────────────────────────────────────────────────

/// Niveles de subcarpetas: cada nivel usa 2 caracteres hex (256 carpetas).
///
/// El nombre tiene 32 caracteres hex; con `L4` se usan 8 y quedan 24 para el
/// nombre del archivo. Es un enum para que un nivel inválido no compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Lv {
    /// Sin subcarpetas: `dir/a3f91c...e7.jpg`
    L0 = 0,
    /// `dir/a3/f91c...e7.jpg` → 256 carpetas
    L1 = 1,
    /// `dir/a3/f9/1c...e7.jpg` → 65.536 carpetas (por defecto)
    #[default]
    L2 = 2,
    /// `dir/a3/f9/1c/2e...e7.jpg` → 16,7 millones de carpetas
    L3 = 3,
    /// `dir/a3/f9/1c/2e/00...e7.jpg` → 4.294 millones de carpetas
    L4 = 4,
}

// ─── LocalStorage ───────────────────────────────────────────────────────────

/// Guarda archivos en `public/storage`, repartidos en subcarpetas para que
/// ninguna carpeta acumule demasiados archivos.
///
/// Cada archivo recibe un nombre aleatorio de 128 bits (32 hex). Los primeros
/// `2 * lv` caracteres se usan como subcarpetas (2 por nivel) y se quitan del nombre:
///
/// ```text
/// Lv::L2: images/avatares/a3/f9/1c2e...e7.jpg
/// Lv::L0: images/avatares/a3f91c2e...e7.jpg
/// ```
///
/// Todas las rutas que recibe y devuelve son relativas a `public/storage`.
/// Este struct solo administra el guardado: validar el archivo y decidir quién
/// puede acceder a él es responsabilidad del controlador o del service.
#[derive(Debug, Clone)]
pub struct LocalStorage {
    root: PathBuf,
    lv: Lv,
}

impl Default for LocalStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalStorage {
    pub fn new() -> Self {
        Self {
            root: PathBuf::from(ROOT),
            lv: Lv::default(),
        }
    }

    /// Cantidad de niveles de subcarpetas (2 caracteres por nivel). Por defecto `Lv::L2`.
    pub fn lv(mut self, lv: Lv) -> Self {
        self.lv = lv;
        self
    }

    /// Guarda `bytes` dentro de `dir` y devuelve la ruta relativa del archivo
    /// (ej. `images/avatares/a3/f9/1c2e...e7.jpg`), lista para guardar en la DB.
    ///
    /// `ext` puede venir con o sin punto (`"jpg"`, `".JPG"`). Si va vacía, el
    /// archivo se guarda sin extensión.
    ///
    /// La escritura es atómica: primero se escribe un `.tmp` y luego se renombra,
    /// así nadie lee un archivo a medio escribir.
    pub async fn save(&self, dir: &str, bytes: &[u8], ext: &str) -> Result<String, ForgeError> {
        let dir = normalize(dir).ok_or_else(|| invalid_path(dir))?;
        let ext = normalize_ext(ext).ok_or_else(|| invalid_path(ext))?;
        let id = random_hex()?;

        let rel = build_rel(&dir, &id, self.lv as u8, &ext);
        let abs = self.root.join(&rel);
        // `rel` siempre termina en un nombre de archivo, así que siempre hay padre.
        let parent = abs.parent().expect("a file path always has a parent directory");

        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| io_error("save", &rel, e))?;

        let tmp = parent.join(format!(".{id}.tmp"));
        if let Err(e) = tokio::fs::write(&tmp, bytes).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(io_error("save", &rel, e));
        }
        if let Err(e) = tokio::fs::rename(&tmp, &abs).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(io_error("save", &rel, e));
        }

        Ok(rel)
    }

    /// Guarda el archivo nuevo y después borra `old`. Devuelve la ruta del nuevo.
    ///
    /// - Si falla el guardado, `old` queda intacto y se devuelve el error.
    /// - Si falla el borrado de `old`, solo se registra en el log: el nuevo ya
    ///   quedó guardado y su ruta es la que importa.
    /// - Si `old` está vacío (ej. el usuario aún no tenía avatar), solo guarda.
    pub async fn replace(&self, old: &str, dir: &str, bytes: &[u8], ext: &str) -> Result<String, ForgeError> {
        let new = self.save(dir, bytes, ext).await?;

        if !old.trim().is_empty()
            && let Err(e) = self.delete(old).await
        {
            log::warning(
                "Failed to delete the previous file while replacing it",
                Some(json!({ "mod": "LocalStorage::replace", "old": old, "new": new, "error": e.message })),
            );
        }

        Ok(new)
    }

    /// Borra el archivo. Si no existe devuelve `Ok`: el resultado es el mismo.
    pub async fn delete(&self, path: &str) -> Result<(), ForgeError> {
        let abs = self.path(path)?;
        match tokio::fs::remove_file(&abs).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_error("delete", path, e)),
        }
    }

    /// Lee el archivo completo en memoria. Devuelve `not_found` si no existe.
    pub async fn get(&self, path: &str) -> Result<Bytes, ForgeError> {
        let abs = self.path(path)?;
        match tokio::fs::read(&abs).await {
            Ok(data) => Ok(Bytes::from(data)),
            Err(e) if e.kind() == ErrorKind::NotFound => Err(ForgeError::not_found()),
            Err(e) => Err(io_error("get", path, e)),
        }
    }

    /// `true` si el archivo existe. Una ruta inválida o un error de lectura cuentan como `false`.
    pub async fn exists(&self, path: &str) -> bool {
        match self.path(path) {
            Ok(abs) => tokio::fs::try_exists(&abs).await.unwrap_or(false),
            Err(_) => false,
        }
    }

    /// Ruta en disco del archivo (ej. `public/storage/images/avatares/a3/f9/...`).
    /// Sirve para enviarlo por partes con `tokio::fs::File` después de revisar permisos.
    pub fn path(&self, path: &str) -> Result<PathBuf, ForgeError> {
        match normalize(path) {
            Some(rel) if !rel.is_empty() => Ok(self.root.join(rel)),
            _ => Err(invalid_path(path)),
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Limpia una ruta relativa: quita `/` al inicio y al final, y une los `//`.
/// `/images//avatares/` → `images/avatares`.
///
/// Devuelve `None` si tiene `.`, `..`, `\` o `\0`, para que nunca se escriba
/// fuera de la raíz del storage. Una ruta vacía devuelve `Some("")`.
fn normalize(path: &str) -> Option<String> {
    let mut parts = Vec::new();
    for seg in path.split('/').filter(|s| !s.is_empty()) {
        if seg == "." || seg == ".." || seg.contains(['\\', '\0']) {
            return None;
        }
        parts.push(seg);
    }
    Some(parts.join("/"))
}

/// `".JPG"` → `"jpg"`. Solo se aceptan letras y números ASCII; vacía es válida.
fn normalize_ext(ext: &str) -> Option<String> {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    ext.chars().all(|c| c.is_ascii_alphanumeric()).then_some(ext)
}

/// Arma `dir/ab/cd/resto.ext` usando los primeros `2 * lv` caracteres de `id` como carpetas.
fn build_rel(dir: &str, id: &str, lv: u8, ext: &str) -> String {
    let cut = lv as usize * 2;
    let mut rel = String::with_capacity(dir.len() + id.len() + lv as usize + ext.len() + 2);

    if !dir.is_empty() {
        rel.push_str(dir);
        rel.push('/');
    }
    for i in 0..lv as usize {
        rel.push_str(&id[i * 2..i * 2 + 2]);
        rel.push('/');
    }
    rel.push_str(&id[cut..]);
    if !ext.is_empty() {
        rel.push('.');
        rel.push_str(ext);
    }
    rel
}

/// 16 bytes aleatorios en hex (32 caracteres, sin guiones).
fn random_hex() -> Result<String, ForgeError> {
    let mut buf = [0u8; 16];
    getrandom::fill(&mut buf).map_err(|e| {
        let msg = "Failed to generate the file name";
        log::error(
            msg,
            Some(json!({ "mod": "LocalStorage::random_hex", "error": e.to_string() })),
        );
        ForgeError::internal().message(msg)
    })?;

    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(32);
    for b in buf {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    Ok(out)
}

fn invalid_path(path: &str) -> ForgeError {
    ForgeError::bad_request().message(format!("Invalid storage path: {path}"))
}

fn io_error(op: &str, path: &str, e: std::io::Error) -> ForgeError {
    let msg = "Storage I/O error";
    log::error(
        msg,
        Some(json!({ "mod": format!("LocalStorage::{op}"), "path": path, "error": e.to_string() })),
    );
    ForgeError::internal().message(msg).caused_by(e)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Storage en una carpeta temporal única, para no tocar `public/storage`.
    fn temp_storage() -> LocalStorage {
        let root = std::env::temp_dir().join(format!("ironforge-storage-{}", random_hex().unwrap()));
        LocalStorage {
            root,
            lv: Lv::default(),
        }
    }

    #[test]
    fn normalize_limpia_slashes() {
        assert_eq!(normalize("/images//avatares/").as_deref(), Some("images/avatares"));
        assert_eq!(normalize("images/avatares").as_deref(), Some("images/avatares"));
        assert_eq!(normalize("/").as_deref(), Some(""));
        assert_eq!(normalize("").as_deref(), Some(""));
    }

    #[test]
    fn normalize_rechaza_traversal() {
        assert_eq!(normalize("images/../../etc"), None);
        assert_eq!(normalize("./images"), None);
        assert_eq!(normalize("images\\avatares"), None);
        assert_eq!(normalize("ima\0ges"), None);
    }

    #[test]
    fn normalize_ext_quita_punto_y_minusculas() {
        assert_eq!(normalize_ext(".JPG").as_deref(), Some("jpg"));
        assert_eq!(normalize_ext("png").as_deref(), Some("png"));
        assert_eq!(normalize_ext("").as_deref(), Some(""));
        assert_eq!(normalize_ext("jpg/x"), None);
        assert_eq!(normalize_ext("tar.gz"), None);
    }

    #[test]
    fn build_rel_por_niveles() {
        let id = "a3f91c2e00000000000000000000ffe7";
        assert_eq!(build_rel("img", id, 0, "jpg"), format!("img/{id}.jpg"));
        assert_eq!(
            build_rel("img", id, 2, "jpg"),
            "img/a3/f9/1c2e00000000000000000000ffe7.jpg"
        );
        assert_eq!(build_rel("img", id, 4, ""), "img/a3/f9/1c/2e/00000000000000000000ffe7");
        assert_eq!(build_rel("", id, 1, "png"), "a3/f91c2e00000000000000000000ffe7.png");
    }

    #[test]
    fn random_hex_32_caracteres() {
        let a = random_hex().unwrap();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, random_hex().unwrap());
    }

    #[test]
    fn lv_por_defecto_y_builder() {
        assert_eq!(LocalStorage::new().lv, Lv::L2);
        assert_eq!(LocalStorage::new().lv(Lv::L4).lv, Lv::L4);
        assert_eq!(Lv::L3 as u8, 3);
    }

    #[tokio::test]
    async fn save_get_replace_delete() {
        let storage = temp_storage();

        let path = storage.save("/images//avatares/", b"hola", ".JPG").await.unwrap();
        assert!(path.starts_with("images/avatares/"));
        assert!(path.ends_with(".jpg"));
        assert_eq!(path.split('/').count(), 5); // images/avatares/ab/cd/nombre.jpg
        assert!(storage.exists(&path).await);
        assert_eq!(storage.get(&path).await.unwrap(), Bytes::from_static(b"hola"));

        let new = storage.replace(&path, "images/avatares", b"chao", "jpg").await.unwrap();
        assert!(!storage.exists(&path).await);
        assert_eq!(storage.get(&new).await.unwrap(), Bytes::from_static(b"chao"));

        // Con `/` al inicio también encuentra el archivo.
        storage.delete(&format!("/{new}")).await.unwrap();
        assert!(!storage.exists(&new).await);
        // Borrar algo que no existe no es error.
        storage.delete(&new).await.unwrap();

        let _ = tokio::fs::remove_dir_all(&storage.root).await;
    }
}
