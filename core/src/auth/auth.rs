use std::sync::Arc;

use crate::{Authenticatable, auth::session::Token, error::ForgeError};
use mongodb::bson::oid::ObjectId;

// === Struct =====================================================

/// Sesión iniciada. Solo existe para usuarios con login.
pub struct Auth<T: Authenticatable> {
    pub session_id: Token,
    pub user_id: ObjectId,
    /// Copia en RAM del usuario (compartida, se obtiene con downcast).
    pub user: Arc<T>,

    pub created_at: i64,
    pub last_activity: i64,
    pub expires_at: i64,
    pub remember: bool,

    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

// Manual para no exigir `T: Clone` (solo se clona el Arc).
impl<T: Authenticatable> Clone for Auth<T> {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id,
            user_id: self.user_id,
            user: Arc::clone(&self.user),
            created_at: self.created_at,
            last_activity: self.last_activity,
            expires_at: self.expires_at,
            remember: self.remember,
            ip: self.ip.clone(),
            user_agent: self.user_agent.clone(),
        }
    }
}

// === Macros de delegacion =======================================

macro_rules! delegate_bool {
    ($($name:ident($arg:ident: $ty:ty)),* $(,)?) => {
        $(
            #[inline]
            pub fn $name(&self, $arg: $ty) -> bool {
                self.user.$name($arg)
            }
        )*
    };
}

macro_rules! delegate_result {
    ($($name:ident($arg:ident: $ty:ty)),* $(,)?) => {
        $(
            #[inline]
            pub fn $name(&self, $arg: $ty) -> Result<(), ForgeError> {
                self.user.$name($arg)
            }
        )*
    };
}

impl<T: Authenticatable> Auth<T> {
    #[inline]
    pub fn user_id(&self) -> ObjectId {
        self.user.auth_id()
    }

    // Roles (bool)
    delegate_bool! {
        has_role(role: &str),
        has_any_role(roles: &[&str]),
        has_all_roles(roles: &[&str]),
        has_not_role(role: &str),
    }

    // Permisos (bool)
    delegate_bool! {
        has_permission(permission: &str),
        has_any(permissions: &[&str]),
        has_all(permissions: &[&str]),
        has_not(permission: &str),
    }

    // Roles (Result)
    delegate_result! {
        can_role(role: &str),
        can_any_role(roles: &[&str]),
        can_all_roles(roles: &[&str]),
        cannot_role(role: &str),
    }

    // Permisos (Result)
    delegate_result! {
        can(permission: &str),
        can_any(permissions: &[&str]),
        can_all(permissions: &[&str]),
        cannot(permission: &str),
    }
}
