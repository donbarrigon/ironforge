use crate::error::HttpError;
use crate::log;
use crate::server::context::Context;
use ahash::AHashMap;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Request, Response};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type ControllerFuture<'a> = Pin<Box<dyn Future<Output = Result<Response<Full<Bytes>>, HttpError>> + Send + 'a>>;
pub type Controller = Arc<dyn for<'a> Fn(&'a mut Context) -> ControllerFuture<'a> + Send + Sync>;

pub type MiddlewareFuture<'a> = Pin<Box<dyn Future<Output = Result<(), HttpError>> + Send + 'a>>;
pub type Middleware = Arc<dyn for<'a> Fn(&'a mut Context) -> MiddlewareFuture<'a> + Send + Sync>;

async fn not_found(_c: &mut Context) -> Result<Response<Full<Bytes>>, HttpError> {
    return Ok(Response::new(Full::new(Bytes::from(""))));
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
    pub map: AHashMap<String, RouteMap>,
}

impl Router {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            static_routes: AHashMap::new(),
            dinamic_routes: Route::new(),
            map: AHashMap::new(),
        }
    }

    pub async fn handle(
        &self,
        req: Request<hyper::body::Incoming>,
        router: Arc<Router>,
    ) -> Result<Response<Full<Bytes>>, Infallible> {
        log::debug(format!("{}:{}", req.method(), req.uri().path()), None);
        Ok(Response::new(Full::new(Bytes::from("Hello, from Forge!"))))
    }
}
