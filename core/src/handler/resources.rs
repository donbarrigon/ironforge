//! Archivos estáticos de `public/resources/` (relativo a donde se ejecuta el servidor).
//!
//! ```ignore
//! r.get("/resources/*", ironforge::handler::resources::serve);
//! // /resources/css/auth/login.css → public/resources/css/auth/login.css
//! ```
//!
//! - Solo extensiones conocidas (css, js, fuentes, imágenes); lo demás es 404.
//! - Nada de `..`, rutas absolutas ni archivos ocultos (`.algo`).
//! - `Cache-Control: no-cache` + `ETag` (tamaño y fecha): el navegador revalida
//!   en cada carga y recibe 304 si el archivo no cambió.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use hyper::body::Bytes;
use hyper::header;

use crate::server::dispatch::WILDCARD_PARAM;
use crate::{Context, ForgeError};

/// Carpeta servida, relativa al directorio de trabajo del servidor.
pub const RESOURCES_DIR: &str = "public/resources";

/// Controller para `GET /resources/*`.
pub async fn serve(c: &mut Context) -> Result<(), ForgeError> {
    let rel = c.get_param(WILDCARD_PARAM).unwrap_or("");
    let (Some(rel), Some(mime)) = (safe_rel(rel), content_type(rel)) else {
        return Err(ForgeError::not_found());
    };
    let path = Path::new(RESOURCES_DIR).join(rel);

    let meta = match tokio::fs::metadata(&path).await {
        Ok(m) if m.is_file() => m,
        _ => return Err(ForgeError::not_found()),
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis());
    let etag = format!("\"{:x}-{:x}\"", meta.len(), modified);

    c.set_header(header::CACHE_CONTROL, "no-cache")?;
    c.set_header(header::ETAG, &etag)?;
    c.set_header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")?;

    let fresh = c
        .r
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|t| t.trim() == etag));
    if fresh {
        return c.response_into(304, Bytes::new());
    }

    let bytes = tokio::fs::read(&path).await.map_err(|_| ForgeError::not_found())?;
    c.set_header(header::CONTENT_TYPE, mime)?;
    c.response_into(200, Bytes::from(bytes))
}

/// Ruta relativa segura: tramos no vacíos de `[A-Za-z0-9._-]`, sin empezar con punto.
fn safe_rel(rel: &str) -> Option<PathBuf> {
    if rel.is_empty() {
        return None;
    }
    let mut out = PathBuf::new();
    for seg in rel.split('/') {
        let ok = !seg.is_empty()
            && !seg.starts_with('.')
            && seg.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if !ok {
            return None;
        }
        out.push(seg);
    }
    Some(out)
}

fn content_type(rel: &str) -> Option<&'static str> {
    let ext = rel.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "map" => "application/json",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_validas() {
        assert_eq!(safe_rel("css/auth/login.css"), Some(PathBuf::from("css/auth/login.css")));
        assert_eq!(safe_rel("js/auth.dart.js"), Some(PathBuf::from("js/auth.dart.js")));
        assert_eq!(safe_rel("fonts/DMSans-Regular.ttf"), Some(PathBuf::from("fonts/DMSans-Regular.ttf")));
    }

    #[test]
    fn rechaza_escapes_y_ocultos() {
        for bad in [
            "", "/", "/etc/passwd", "../env.json", "css/../../env.json", "css//a.css", ".env", "css/.hidden.css",
            "css/a b.css", "css\\a.css", "css/%2e%2e/a.css", "css/a.css/",
        ] {
            assert_eq!(safe_rel(bad), None, "{bad}");
        }
    }

    #[test]
    fn solo_extensiones_conocidas() {
        assert_eq!(content_type("a.CSS"), Some("text/css; charset=utf-8"));
        assert_eq!(content_type("a.dart.js"), Some("text/javascript; charset=utf-8"));
        assert_eq!(content_type("env.json"), None);
        assert_eq!(content_type("main.dart"), None);
        assert_eq!(content_type("sin_extension"), None);
    }
}
