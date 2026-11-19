//! Tokens CSRF firmados con `app.key` (sin estado: no se guardan en RAM ni en Mongo).
//!
//! Token = base64url(nonce ‖ expires_at ‖ HMAC-SHA256(app.key, "csrf" ‖ nonce ‖ expires_at ‖ atadura)).
//! - `nonce`: 16 bytes aleatorios, cada token es distinto.
//! - `expires_at`: segundos unix (i64 big-endian); vida = `session.csrf_ttl_minutes`.
//! - `atadura`: a qué pertenece el token. No viaja en el token, solo entra en la firma,
//!   así que un token de una sesión no sirve en otra.
//!   - `csrf(ctx)`: la sesión del request (o "invitado" si no hay sesión).
//!   - `csrf_with(valor)`: un texto cualquiera que elige quien llama.

use std::sync::OnceLock;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::Sha256;

use crate::auth::session::{Token, decode_app_key, session};
use crate::config::env;
use crate::{Context, ForgeError, log};

type HmacSha256 = Hmac<Sha256>;

const DOMAIN: &[u8] = b"csrf";
const NONCE_LEN: usize = 16;
const EXP_LEN: usize = 8;
const MAC_LEN: usize = 32;
const PAYLOAD_LEN: usize = NONCE_LEN + EXP_LEN;
const TOKEN_LEN: usize = PAYLOAD_LEN + MAC_LEN;

/// A qué queda atado un token. Cada variante firma con su propia etiqueta,
/// así un `csrf_with(...)` nunca puede validar como token de sesión ni al revés.
#[derive(Clone, Copy)]
enum Binding<'a> {
    Guest,
    Session(&'a Token),
    Value(&'a [u8]),
}

impl Binding<'_> {
    fn feed(&self, mac: &mut HmacSha256) {
        let (tag, data): (u8, &[u8]) = match self {
            Binding::Guest => (0, &[]),
            Binding::Session(token) => (1, &token[..]),
            Binding::Value(value) => (2, value),
        };
        mac.update(&[tag]);
        mac.update(&(data.len() as u64).to_be_bytes());
        mac.update(data);
    }
}

// === API ========================================================

/// Token CSRF atado a la sesión del request.
/// Sin sesión (invitado, ej. formulario de login) queda atado a "invitado"
/// y deja de valer cuando el usuario inicia sesión.
pub fn csrf(ctx: &Context) -> Result<String, ForgeError> {
    let token = ctx_session_id(ctx)?;
    new_token(binding(&token))
}

/// `true` si el token es de la sesión de este request, tiene firma válida y no expiró.
pub fn validate_csrf(ctx: &Context, token: impl AsRef<str>) -> bool {
    let Ok(session_id) = ctx_session_id(ctx) else {
        return false;
    };
    check(token.as_ref(), binding(&session_id))
}

/// Token CSRF atado a un valor propio (ej. un id de formulario o de recurso).
pub fn csrf_with(value: impl AsRef<str>) -> Result<String, ForgeError> {
    new_token(Binding::Value(value.as_ref().as_bytes()))
}

/// `true` si el token se generó con `csrf_with(value)`, tiene firma válida y no expiró.
pub fn validate_csrf_with(token: impl AsRef<str>, value: impl AsRef<str>) -> bool {
    check(token.as_ref(), Binding::Value(value.as_ref().as_bytes()))
}

// === Helpers ====================================================

/// `app.key` decodificada una sola vez.
static KEY: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();

fn key() -> Result<&'static [u8], ForgeError> {
    KEY.get_or_init(|| decode_app_key(&env().app.key))
        .as_deref()
        .map_err(|e| ForgeError::internal().message(e.clone()))
}

fn ctx_session_id(ctx: &Context) -> Result<Option<Token>, ForgeError> {
    Ok(ctx.current_token(session()?))
}

fn binding(session_id: &Option<Token>) -> Binding<'_> {
    match session_id {
        Some(token) => Binding::Session(token),
        None => Binding::Guest,
    }
}

fn new_token(binding: Binding) -> Result<String, ForgeError> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|e| {
        let msg = "Failed to generate csrf token";
        log::error(msg, Some(json!({ "mod": "csrf::new_token", "error": e.to_string() })));
        ForgeError::internal().message(msg)
    })?;
    let expires_at = now_seconds() + env().session.csrf_ttl().as_secs() as i64;
    Ok(sign(key()?, &nonce, expires_at, binding))
}

fn check(token: &str, binding: Binding) -> bool {
    match key() {
        Ok(key) => verify(key, token, binding, now_seconds()),
        Err(_) => false,
    }
}

fn mac(key: &[u8], payload: &[u8], binding: Binding) -> HmacSha256 {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC acepta claves de cualquier largo");
    mac.update(DOMAIN);
    mac.update(payload);
    binding.feed(&mut mac);
    mac
}

fn sign(key: &[u8], nonce: &[u8; NONCE_LEN], expires_at: i64, binding: Binding) -> String {
    let mut raw = [0u8; TOKEN_LEN];
    raw[..NONCE_LEN].copy_from_slice(nonce);
    raw[NONCE_LEN..PAYLOAD_LEN].copy_from_slice(&expires_at.to_be_bytes());
    let tag = mac(key, &raw[..PAYLOAD_LEN], binding).finalize().into_bytes();
    raw[PAYLOAD_LEN..].copy_from_slice(&tag);
    URL_SAFE_NO_PAD.encode(raw)
}

fn verify(key: &[u8], token: &str, binding: Binding, now: i64) -> bool {
    let Ok(raw) = URL_SAFE_NO_PAD.decode(token.trim()) else {
        return false;
    };
    if raw.len() != TOKEN_LEN {
        return false;
    }
    // Primero la firma: no se confía en `expires_at` hasta saber que no fue alterado.
    if mac(key, &raw[..PAYLOAD_LEN], binding).verify_slice(&raw[PAYLOAD_LEN..]).is_err() {
        return false;
    }
    let exp: [u8; EXP_LEN] = raw[NONCE_LEN..PAYLOAD_LEN].try_into().expect("largo fijo");
    now < i64::from_be_bytes(exp)
}

fn now_seconds() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &[u8] = b"clave-de-prueba-a-de-32-bytes!!!";
    const KEY_B: &[u8] = b"clave-de-prueba-b-de-32-bytes!!!";
    const NOW: i64 = 1_800_000_000;
    const SID_A: Token = [1u8; 32];
    const SID_B: Token = [2u8; 32];

    fn token(key: &[u8], expires_at: i64, binding: Binding) -> String {
        sign(key, &[7u8; NONCE_LEN], expires_at, binding)
    }

    #[test]
    fn valido_antes_de_expirar() {
        let t = token(KEY_A, NOW + 60, Binding::Session(&SID_A));
        assert!(verify(KEY_A, &t, Binding::Session(&SID_A), NOW));
        assert!(verify(KEY_A, &format!("  {t}\n"), Binding::Session(&SID_A), NOW));
    }

    #[test]
    fn expirado() {
        let t = token(KEY_A, NOW, Binding::Guest);
        assert!(!verify(KEY_A, &t, Binding::Guest, NOW));
        assert!(!verify(KEY_A, &t, Binding::Guest, NOW + 1));
    }

    #[test]
    fn otra_clave() {
        let t = token(KEY_A, NOW + 60, Binding::Guest);
        assert!(!verify(KEY_B, &t, Binding::Guest, NOW));
    }

    #[test]
    fn atado_a_su_sesion() {
        let t = token(KEY_A, NOW + 60, Binding::Session(&SID_A));
        assert!(!verify(KEY_A, &t, Binding::Session(&SID_B), NOW));
        assert!(!verify(KEY_A, &t, Binding::Guest, NOW));
    }

    #[test]
    fn invitado_no_sirve_con_sesion() {
        let t = token(KEY_A, NOW + 60, Binding::Guest);
        assert!(verify(KEY_A, &t, Binding::Guest, NOW));
        assert!(!verify(KEY_A, &t, Binding::Session(&SID_A), NOW));
    }

    #[test]
    fn atado_a_su_valor() {
        let t = token(KEY_A, NOW + 60, Binding::Value(b"form:perfil"));
        assert!(verify(KEY_A, &t, Binding::Value(b"form:perfil"), NOW));
        assert!(!verify(KEY_A, &t, Binding::Value(b"form:otro"), NOW));
        assert!(!verify(KEY_A, &t, Binding::Value(b""), NOW));
    }

    #[test]
    fn valor_no_se_confunde_con_sesion_ni_invitado() {
        // Mismos bytes que el session_id, pero como valor: firma distinta.
        let t = token(KEY_A, NOW + 60, Binding::Value(&SID_A));
        assert!(!verify(KEY_A, &t, Binding::Session(&SID_A), NOW));
        let t = token(KEY_A, NOW + 60, Binding::Value(b""));
        assert!(!verify(KEY_A, &t, Binding::Guest, NOW));
    }

    #[test]
    fn alterado() {
        let b = Binding::Session(&SID_A);
        let t = token(KEY_A, NOW + 60, b);
        let mut raw = URL_SAFE_NO_PAD.decode(&t).unwrap();
        // Extender la expiración sin re-firmar.
        raw[NONCE_LEN..PAYLOAD_LEN].copy_from_slice(&(NOW + 999_999).to_be_bytes());
        assert!(!verify(KEY_A, &URL_SAFE_NO_PAD.encode(&raw), b, NOW));
        // Cambiar un byte de la firma.
        let mut raw = URL_SAFE_NO_PAD.decode(&t).unwrap();
        raw[TOKEN_LEN - 1] ^= 1;
        assert!(!verify(KEY_A, &URL_SAFE_NO_PAD.encode(&raw), b, NOW));
    }

    #[test]
    fn basura() {
        for t in ["", "abc", "%%%", &"A".repeat(75), &"A".repeat(200)] {
            assert!(!verify(KEY_A, t, Binding::Guest, NOW), "{t}");
        }
    }

    #[test]
    fn nonces_distintos_tokens_distintos() {
        let a = sign(KEY_A, &[1u8; NONCE_LEN], NOW + 60, Binding::Guest);
        let b = sign(KEY_A, &[2u8; NONCE_LEN], NOW + 60, Binding::Guest);
        assert_ne!(a, b);
        assert!(verify(KEY_A, &a, Binding::Guest, NOW) && verify(KEY_A, &b, Binding::Guest, NOW));
    }
}
