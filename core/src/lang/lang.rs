use ahash::AHashMap;
use serde_json::Value;
use std::sync::OnceLock;

use crate::log;

// ─── Singleton ────────────────────────────────────────────────────────────────

static TRANSLATIONS: OnceLock<AHashMap<String, AHashMap<String, String>>> = OnceLock::new();

pub fn load_translations() -> Result<(), Box<dyn std::error::Error>> {
    let lang_dir = std::path::Path::new("config/lang");

    if !lang_dir.exists() {
        let _ = TRANSLATIONS.set(AHashMap::new());
        return Ok(());
    }

    let mut all: AHashMap<String, AHashMap<String, String>> = AHashMap::new();

    for entry in std::fs::read_dir(lang_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let locale = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid lang filename")?
            .to_string();

        let content = std::fs::read_to_string(&path)?;
        let map: AHashMap<String, String> = serde_json::from_str(&content)?;
        all.insert(locale, map);
    }

    TRANSLATIONS
        .set(all)
        .map_err(|_| "TRANSLATIONS has already been initialized".into())
}

// ─── Funciones publicas ───────────────────────────────────────────────────────

/// Traduccion sin parametros
/// Si no encuentra la traduccion retorna el mismo string que entro
pub fn t(locale: impl Into<String>, key: impl Into<String>) -> String {
    let key = key.into();

    let Some(all) = TRANSLATIONS.get() else {
        log::warning(
            "TRANSLATIONS is not initialized — call load_translations() before accessing lang",
            None,
        );
        return key;
    };

    all.get(&locale.into())
        .and_then(|map| map.get(&key))
        .cloned()
        .unwrap_or(key)
}

/// Traduccion con parametros — reemplaza {placeholders}
/// Si no encuentra la traduccion retorna el mismo string que entro con los placeholders reemplazados
pub fn tr(locale: impl Into<String>, key: impl Into<String>, params: &Value) -> String {
    let key = key.into();

    let Some(all) = TRANSLATIONS.get() else {
        log::warning(
            "TRANSLATIONS is not initialized — call load_translations() before accessing lang",
            None,
        );
        return replace_placeholders(&key, params);
    };

    let template = all
        .get(&locale.into())
        .and_then(|map| map.get(&key))
        .map(|s| s.as_str())
        .unwrap_or(&key);

    replace_placeholders(template, params)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn replace_placeholders(template: &str, params: &Value) -> String {
    let mut result = template.to_string();

    if let Value::Object(map) = params {
        for (key, value) in map {
            let placeholder = format!("{{{}}}", key);
            let replacement = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
    }

    result
}
