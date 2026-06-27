use std::{convert::Infallible, sync::Arc};

use ahash::AHashMap;
use http_body_util::Full;
use hyper::{Response, body::Bytes};

use crate::{
    HttpError,
    error::Empty,
    cluster::{
        Request, Router,
        router::{Param, Route, RouteMap},
    },
};

pub async fn handler(
    router: Arc<Router>,
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = req.method().to_string();
    let path = req.uri().path().to_string().trim_matches('/').to_lowercase();

    let key = format!("{}/{}", path, method);
    if let Some(route) = router.static_routes.get(&key) {
        return run_route(req, vec![], &route, Arc::clone(&router.map)).await;
    }

    Ok(HttpError::not_found("Not found", Empty).response())
}

async fn run_route(
    req: hyper::Request<hyper::body::Incoming>,
    params: Vec<Param>,
    route: &Route,
    map: Arc<AHashMap<String, RouteMap>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let mut request = Request::new(req, params, map);
    for middleware in &route.middlewares {
        match middleware(&mut request).await {
            Ok(()) => continue,
            Err(e) => {
                return Ok(e.response());
            }
        }
    }
    match (route.controller)(&mut request).await {
        Ok(res) => Ok(res),
        Err(e) => Ok(e.response()),
    }
}
