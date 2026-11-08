use std::fmt;

use mongodb::bson::oid::ObjectId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::error::ForgeError;
use crate::log;

/// `Serialize + DeserializeOwned`: el usuario se guarda en el documento de sesión
/// en Mongo y se reconstruye al recargar una sesión que no está en RAM.
pub trait Authenticatable: Serialize + DeserializeOwned + Send + Sync + 'static {
    // === Obligatorios: los implementa el modelo =================

    fn auth_id(&self) -> ObjectId;
    fn roles(&self) -> &[String];
    fn permissions(&self) -> &[String];

    // === Roles (bool) ===========================================

    fn has_role(&self, role: &str) -> bool {
        self.roles().iter().any(|r| r == role)
    }

    fn has_any_role(&self, roles: &[&str]) -> bool {
        roles.iter().any(|r| self.has_role(r))
    }

    fn has_all_roles(&self, roles: &[&str]) -> bool {
        roles.iter().all(|r| self.has_role(r))
    }

    fn has_not_role(&self, role: &str) -> bool {
        !self.has_role(role)
    }

    // === Roles (Result) =========================================

    /// Ok si tiene el rol, 403 si no.
    fn can_role(&self, role: &str) -> Result<(), ForgeError> {
        if self.has_role(role) {
            Ok(())
        } else {
            Err(deny("can_role", json!(role), format!("missing role `{role}`")))
        }
    }

    /// Ok si tiene al menos uno de los roles.
    fn can_any_role(&self, roles: &[&str]) -> Result<(), ForgeError> {
        if self.has_any_role(roles) {
            Ok(())
        } else {
            Err(deny(
                "can_any_role",
                json!(roles),
                format!("missing any of roles `{}`", roles.join("`, `")),
            ))
        }
    }

    /// Ok si tiene todos los roles; el detalle indica el primero que falta.
    fn can_all_roles(&self, roles: &[&str]) -> Result<(), ForgeError> {
        match roles.iter().find(|r| self.has_not_role(r)) {
            None => Ok(()),
            Some(missing) => Err(deny("can_all_roles", json!(roles), format!("missing role `{missing}`"))),
        }
    }

    /// Ok si NO tiene el rol, 403 si lo tiene.
    fn cannot_role(&self, role: &str) -> Result<(), ForgeError> {
        if self.has_not_role(role) {
            Ok(())
        } else {
            Err(deny(
                "cannot_role",
                json!(role),
                format!("not allowed with role `{role}`"),
            ))
        }
    }

    // === Permisos (bool) ========================================

    fn has_permission(&self, permission: &str) -> bool {
        self.permissions()
            .iter()
            .any(|granted| permission_matches(granted, permission))
    }

    fn has_any(&self, permissions: &[&str]) -> bool {
        permissions.iter().any(|p| self.has_permission(p))
    }

    fn has_all(&self, permissions: &[&str]) -> bool {
        permissions.iter().all(|p| self.has_permission(p))
    }

    fn has_not(&self, permission: &str) -> bool {
        !self.has_permission(permission)
    }

    // === Permisos (Result) ======================================

    /// Ok si tiene el permiso, 403 si no.
    fn can(&self, permission: &str) -> Result<(), ForgeError> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(deny(
                "can",
                json!(permission),
                format!("missing permission `{permission}`"),
            ))
        }
    }

    /// Ok si tiene al menos uno de los permisos.
    fn can_any(&self, permissions: &[&str]) -> Result<(), ForgeError> {
        if self.has_any(permissions) {
            Ok(())
        } else {
            Err(deny(
                "can_any",
                json!(permissions),
                format!("missing any of permissions `{}`", permissions.join("`, `")),
            ))
        }
    }

    /// Ok si tiene todos los permisos; el detalle indica el primero que falta.
    fn can_all(&self, permissions: &[&str]) -> Result<(), ForgeError> {
        match permissions.iter().find(|p| self.has_not(p)) {
            None => Ok(()),
            Some(missing) => Err(deny(
                "can_all",
                json!(permissions),
                format!("missing permission `{missing}`"),
            )),
        }
    }

    /// Ok si NO tiene el permiso, 403 si lo tiene.
    fn cannot(&self, permission: &str) -> Result<(), ForgeError> {
        if self.has_not(permission) {
            Ok(())
        } else {
            Err(deny(
                "cannot",
                json!(permission),
                format!("not allowed with permission `{permission}`"),
            ))
        }
    }
}

// === Helpers privados ===========================================

/// Error interno que solo envuelve el texto del motivo para poder pasarlo a `caused_by`.
#[derive(Debug)]
struct AuthDenied(String);

impl fmt::Display for AuthDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AuthDenied {}

/// Registra el motivo con `log::warning` y devuelve un 403 con el mensaje por defecto;
/// el detalle solo queda en `caused_by` (visible en modo debug).
fn deny(func: &str, input: Value, reason: String) -> ForgeError {
    log::warning(reason.clone(), Some(json!({ "fn": func, "input": input })));
    ForgeError::forbidden().caused_by(AuthDenied(reason))
}

/// `*` concede todo, `posts.*` concede `posts.create` y `posts.comments.create`,
/// y el resto se compara exacto.
fn permission_matches(granted: &str, required: &str) -> bool {
    if granted == "*" || granted == required {
        return true;
    }
    match granted.strip_suffix(".*") {
        Some(prefix) => required.strip_prefix(prefix).is_some_and(|rest| rest.starts_with('.')),
        None => false,
    }
}
