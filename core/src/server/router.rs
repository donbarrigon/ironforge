use crate::error::HttpError;
use crate::handler::context::Context;
use ahash::AHashMap;
use hyper::body::Bytes;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// === Types =====================================================

// pub type ControllerFuture<'a> = Pin<Box<dyn Future<Output = Result<Response<Full<Bytes>>, HttpError>> + Send + 'a>>;
// pub type Controller = Arc<dyn for<'a> Fn(&'a mut Request) -> ControllerFuture<'a> + Send + Sync>;

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

// === RouteMap ==================================================

#[derive(Clone)]
pub struct RouteMap {
    pub path: String,
    pub method: String,
    pub params: Vec<String>,
}

// === Route =====================================================

#[derive(Clone)]
pub struct Route {
    pub controller: Controller,
    pub middlewares: Vec<Middleware>,
    pub params: Vec<String>,
    pub static_routes: AHashMap<String, Route>,
    pub dinamic_routes: Option<Box<Route>>,
    pub is_dinamic: bool,
    pub is_wildcard: bool,
}

impl Route {
    pub fn new() -> Self {
        Self {
            controller: Arc::new(|c| Box::pin(default_not_found(c))),
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
    pub static_routes: AHashMap<String, Route>,
    pub dinamic_routes: Route,
    pub map: Arc<AHashMap<String, RouteMap>>,
    pub not_found_controller: Controller,
}

impl Router {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            static_routes: AHashMap::new(),
            dinamic_routes: Route::new(),
            map: Arc::new(AHashMap::new()),
            not_found_controller: Arc::new(|c| Box::pin(default_not_found(c))),
        }
    }

    pub fn set_not_found(mut self, controller: Controller) -> Self {
        self.not_found_controller = controller;
        self
    }
}

pub async fn default_not_found(c: &mut Context) -> Result<(), HttpError> {
    c.reply(404, Bytes::new())
}
