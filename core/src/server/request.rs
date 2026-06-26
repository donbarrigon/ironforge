use ahash::AHashMap;
use hyper::body::Incoming;
use std::sync::Arc;

use crate::server::{
    Router,
    router::{Param, RouteMap},
};

pub struct Request {
    pub inner: hyper::Request<Incoming>,
    pub params: Vec<Param>,
    pub map: Arc<AHashMap<String, RouteMap>>,
}

impl Request {
    pub fn new(req: hyper::Request<Incoming>, params: Vec<Param>, map: Arc<AHashMap<String, RouteMap>>) -> Self {
        Self {
            inner: req,
            params,
            map,
        }
    }
}
