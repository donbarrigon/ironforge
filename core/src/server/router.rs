use crate::error::HttpError;
use crate::handler::context::Context;
use ahash::AHashMap;
use hyper::body::Bytes;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// === Types =====================================================

pub type BoxFuture<'a> = Pin<Box<dyn Future<Output = Result<(), HttpError>> + Send + 'a>>;

/// Tanto Controller como Middleware usan el mismo tipo base
pub type Controller = Arc<dyn for<'a> Fn(&'a mut Context) -> BoxFuture<'a> + Send + Sync>;
pub type Middleware = Controller;

// === Params ====================================================

#[derive(Clone)]
pub struct Param {
    pub name: String,
    pub value: String,
}

pub type Params = Vec<Param>;

pub trait QueryParams {
    fn require(&self, name: &str) -> Result<&str, HttpError>;
    fn get(&self, name: &str) -> Option<&str>;
    fn get_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str;
}

impl QueryParams for Params {
    fn require(&self, name: &str) -> Result<&str, HttpError> {
        self.iter()
            .find(|p| p.name == name)
            .map(|p| p.value.as_str())
            .ok_or_else(|| HttpError::bad_request(format!("missing param `{}`", name)))
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.iter().find(|p| p.name == name).map(|p| p.value.as_str())
    }

    fn get_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.get(name).unwrap_or(default)
    }
}

// === Segment ====================================================
// Antes se llamaba Route. Cada nodo del árbol representa un
// segmento de la ruta (ej. "users", "{id}", "*"), no la ruta
// completa — de ahí el rename.

#[derive(Clone)]
pub struct Segment {
    pub controller: Controller,
    pub middlewares: Vec<Middleware>,
    pub params: Vec<String>,
    pub static_routes: AHashMap<String, Segment>,
    pub dinamic_routes: Option<Box<Segment>>,
    pub is_dinamic: bool,
    pub is_wildcard: bool,
}

impl Segment {
    /// Crea un nodo "vacío" que ya apunta al controller not_found
    /// pasado por el builder. Reemplaza al viejo Route::new() que
    /// siempre apuntaba a un default_not_found fijo.
    pub fn new(not_found: Controller) -> Self {
        Self {
            controller: not_found,
            middlewares: Vec::new(),
            params: Vec::new(),
            static_routes: AHashMap::new(),
            dinamic_routes: None,
            is_dinamic: false,
            is_wildcard: false,
        }
    }
}

// === Router ====================================================

#[derive(Clone)]
pub struct Router {
    pub name: String,
    pub static_routes: AHashMap<String, Segment>,
    pub dinamic_routes: Segment,
    /// key: nombre de la ruta (route name)
    /// value: "METHOD:/path/con/:params" en un solo string,
    /// ej. "GET:/api/users/:id/show"
    pub map: Arc<AHashMap<String, String>>,
    pub not_found_controller: Controller,
}

impl Router {
    pub fn new(name: impl Into<String>, not_found: Controller) -> Self {
        Self {
            name: name.into(),
            static_routes: AHashMap::new(),
            dinamic_routes: Segment::new(not_found.clone()),
            map: Arc::new(AHashMap::new()),
            not_found_controller: not_found,
        }
    }
}

pub async fn default_not_found(c: &mut Context) -> Result<(), HttpError> {
    c.reply(404, Bytes::new())
}
