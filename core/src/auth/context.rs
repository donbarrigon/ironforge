//! Atajos de sesión sobre `Context`: login, auth, logout y regenerate.
//!
//! - La sesión se resuelve una vez por request y queda en `extensions` como `Auth<T>`.
//! - Cada vez que se resuelve, se renueva en RAM (`Session::get`) y se reenvía la
//!   cookie con `Max-Age` = lo que le queda a la sesión, con o sin "recordarme".

use hyper::header::{self, HeaderValue};

use crate::auth::session::{Session, Token, session};
use crate::config::env;
use crate::{Auth, Authenticatable, Context, ForgeError};

/// Marca que la sesión se cerró en este request: la cookie del request ya no vale.
#[derive(Clone)]
struct SignedOut;

impl Context {
    /// Usuario autenticado o 401.
    pub async fn auth<T: Authenticatable>(&mut self) -> Result<Auth<T>, ForgeError> {
        self.try_auth::<T>().await?.ok_or_else(ForgeError::unauthorized)
    }

    /// Usuario autenticado si lo hay (rutas con login opcional).
    /// Solo falla si `Session` no se inicializó o no se pudo armar la cookie.
    pub async fn try_auth<T: Authenticatable>(&mut self) -> Result<Option<Auth<T>>, ForgeError> {
        let session = session()?;
        let Some(token) = self.current_token(session) else {
            return Ok(None);
        };
        if let Some(auth) = self.ext::<Auth<T>>().filter(|a| a.session_id == token) {
            return Ok(Some(auth.clone()));
        }

        let Some(mut auth) = session.get::<T>(&token).await else {
            return Ok(None);
        };
        auth.user_agent = self.user_agent();
        self.put_session_cookie(session.set_cookie_header(&token, auth.expires_at))?;
        self.session_id = Some(token);
        self.set_ext(auth.clone());
        Ok(Some(auth))
    }

    /// Crea la sesión y pone la cookie. Si el request ya traía una sesión, la cierra
    /// para que el token viejo no siga vivo.
    pub async fn login<T: Authenticatable>(&mut self, user: T, remember: bool) -> Result<Auth<T>, ForgeError> {
        let session = session()?;
        if let Some(old) = self.current_token(session) {
            session.destroy(&old).await?;
        }

        let mut auth = session.create(user, remember).await?;
        auth.user_agent = self.user_agent();
        self.put_session_cookie(session.set_cookie_header(&auth.session_id, auth.expires_at))?;
        self.session_id = Some(auth.session_id);
        self.extensions.remove::<SignedOut>();
        self.set_ext(auth.clone());
        Ok(auth)
    }

    /// Cierra la sesión y borra la cookie. Sin sesión no es error.
    pub async fn logout(&mut self) -> Result<(), ForgeError> {
        let session = session()?;
        if let Some(token) = self.current_token(session) {
            session.destroy(&token).await?;
        }
        self.session_id = None;
        self.set_ext(SignedOut);
        self.put_session_cookie(session.clear_cookie_header())
    }

    /// Token nuevo para la sesión actual (después de un cambio de privilegios) o 401.
    pub async fn regenerate<T: Authenticatable>(&mut self) -> Result<Auth<T>, ForgeError> {
        let session = session()?;
        let mut auth = self.auth::<T>().await?;
        let new = session.regenerate(&auth.session_id).await?;

        auth.session_id = new;
        self.put_session_cookie(session.set_cookie_header(&new, auth.expires_at))?;
        self.session_id = Some(new);
        self.set_ext(auth.clone());
        Ok(auth)
    }

    // === Privadas ===============================================

    /// Token de este request: el ya resuelto o el de la cookie, salvo que se haya hecho logout.
    fn current_token(&self, session: &Session) -> Option<Token> {
        if self.ext::<SignedOut>().is_some() {
            return None;
        }
        self.session_id.or_else(|| self.cookie_token(session))
    }

    /// Busca `cookie_name` en los headers `Cookie: a=1; b=2` y verifica la firma.
    fn cookie_token(&self, session: &Session) -> Option<Token> {
        let name = &env().session.cookie_name;
        self.r
            .headers()
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .filter_map(|pair| pair.trim().split_once('='))
            .find(|(k, _)| k == name)
            .and_then(|(_, value)| session.decode_cookie(value))
    }

    fn user_agent(&self) -> Option<String> {
        self.r
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    }

    /// Reemplaza el `Set-Cookie` de la sesión sin tocar otras cookies de la respuesta.
    fn put_session_cookie(&mut self, value: String) -> Result<(), ForgeError> {
        let value = HeaderValue::from_str(&value)
            .map_err(|e| ForgeError::internal().message("invalid session Set-Cookie").caused_by(e))?;
        let prefix = format!("{}=", env().session.cookie_name);
        let others: Vec<HeaderValue> = self
            .headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter(|v| !v.as_bytes().starts_with(prefix.as_bytes()))
            .cloned()
            .collect();

        self.headers.remove(header::SET_COOKIE);
        for v in others {
            self.headers.append(header::SET_COOKIE, v);
        }
        self.headers.append(header::SET_COOKIE, value);
        Ok(())
    }
}
