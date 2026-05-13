use crate::errors::Error;
use crate::log;
use crate::server::context::Context;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Request, Response};
use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type ControllerFuture = Pin<Box<dyn Future<Output = Result<Response<Full<Bytes>>, Error>> + Send>>;
pub type Controller = Arc<dyn Fn(&mut Context) -> ControllerFuture + Send + Sync>;

pub type MiddlewareFuture = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;
pub type Middleware = Arc<dyn Fn(&mut Context) -> MiddlewareFuture + Send + Sync>;

#[derive(Clone)]
pub struct Param {
    pub name: String,
    pub value: String,
}

#[derive(Clone)]
pub struct Route {
    pub path: &'static str,
    pub controller: Option<Controller>,
    pub middlewares: Vec<Middleware>,
    pub params: Vec<Param>,
    pub children: Vec<Route>,
    pub is_dynamic: bool,
}

impl Route {
    pub fn new() -> Self {
        Self {
            path: "",
            controller: None,
            middlewares: Vec::new(),
            params: Vec::new(),
            children: Vec::new(),
            is_dynamic: false,
        }
    }
}

#[derive(Clone)]
pub struct Router {
    pub route: Route,
    pub map: HashMap<String, Route>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            route: Route::new(),
            map: HashMap::new(),
        }
    }

    pub async fn handle(&self, req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
        log::debug(format!("{}:{}", req.method(), req.uri().path()), None);
        Ok(Response::new(Full::new(Bytes::from("Hello, from Forge!"))))
    }
}
