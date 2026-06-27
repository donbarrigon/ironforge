use crate::cluster::request::Request;
use crate::error::HttpError;
use ahash::AHashMap;
use http_body_util::Full;
use hyper::Response;
use hyper::body::Bytes;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type ControllerFuture<'a> = Pin<Box<dyn Future<Output = Result<Response<Full<Bytes>>, HttpError>> + Send + 'a>>;
pub type Controller = Arc<dyn for<'a> Fn(&'a mut Request) -> ControllerFuture<'a> + Send + Sync>;

pub type MiddlewareFuture<'a> = Pin<Box<dyn Future<Output = Result<(), HttpError>> + Send + 'a>>;
pub type Middleware = Arc<dyn for<'a> Fn(&'a mut Request) -> MiddlewareFuture<'a> + Send + Sync>;

async fn not_found(_c: &mut Request) -> Result<Response<Full<Bytes>>, HttpError> {
    return Ok(Response::new(Full::new(Bytes::from("404"))));
}

#[derive(Clone)]
pub struct Param {
    pub name: String,
    pub value: String,
}

#[derive(Clone)]
pub struct RouteMap {
    pub path: String,
    pub method: String,
    pub params: Vec<String>,
}

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
            controller: Arc::new(|c| Box::pin(not_found(c))),
            middlewares: Vec::new(),
            params: Vec::new(),
            static_routes: AHashMap::new(),
            dinamic_routes: None,
            is_dinamic: false,
            is_wildcard: false,
        }
    }
}

#[derive(Clone)]
pub struct Router {
    pub name: String,
    pub static_routes: AHashMap<String, Route>,
    pub dinamic_routes: Route,
    pub map: Arc<AHashMap<String, RouteMap>>,
}

impl Router {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            static_routes: AHashMap::new(),
            dinamic_routes: Route::new(),
            map: Arc::new(AHashMap::new()),
        }
    }
}
