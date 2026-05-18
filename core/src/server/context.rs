use hyper::Request;
use hyper::body::Incoming;
use std::sync::Arc;

use crate::server::{
    Router,
    router::{Param, Route, RouteMap},
};

pub struct Context {
    pub req: Request<Incoming>,
    pub params: Vec<Param>,
    pub router: Arc<Router>,
}

impl Context {
    pub fn new(req: Request<Incoming>, params: Vec<Param>, router: Arc<Router>) -> Self {
        Self { req, params, router }
    }
}
