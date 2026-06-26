use std::{convert::Infallible, sync::Arc};

use ahash::AHashMap;
use http_body_util::Full;
use hyper::{Response, body::Bytes};

use crate::{
    log,
    server::{
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

    log::debug(format!("{}:{}", req.method(), req.uri().path()), None);
    Ok(Response::new(Full::new(Bytes::from("Hello, from Forge!"))))
}

async fn run_route(
    req: hyper::Request<hyper::body::Incoming>,
    params: Vec<Param>,
    route: &Route,
    map: Arc<AHashMap<String, RouteMap>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let mut req = Request::new(req, params, map);
    // TODO: correr los middlewares
    // TODO: correr el controller
    // TODO: manejar los errores para respuesta automatica
    Ok(Response::new(Full::new(Bytes::from("Hello, from Forge!"))))
}
