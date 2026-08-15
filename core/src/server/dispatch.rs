use hyper::body::Incoming;
use hyper::{Request, Response};
use std::convert::Infallible;

use crate::handler::ResBody;
use crate::handler::context::Context;
use crate::log;
use crate::server::router::Router;

pub async fn dispatch(req: Request<Incoming>, router: &'static Router) -> Result<Response<ResBody>, Infallible> {
    let mut c = Context::new(req);

    let url =
        c.r.uri()
            .path()
            .to_string()
            .to_lowercase()
            .trim_start_matches("/")
            .to_string();
    let method = c.r.method().to_string();
    let key = format!("{}/{}", url, method);
    if let Some(segment) = router.static_routes.get(&key) {
        for handler in &segment.handlers {
            if let Err(e) = handler(&mut c).await {
                if let Err(ex) = c.response_error(e) {
                    let exv = serde_json::to_value(&ex).ok();
                    log::error("Failed to send error", exv);
                }
                break;
            }
        }
        return Ok(c.into_response());
    }

    let mut segment = &router.dinamic_routes;
    for k in key.split('/') {
        if let Some(seg) = segment.static_routes.get(k) {
            segment = &seg;
        } else if let Some(seg) = segment.dinamic_routes.as_ref() {
            segment = &seg;
        } else {
            // No se encontró la ruta, se ejecuta el controller not_found
            if let Err(e) = (router.not_found_controller)(&mut c).await {
                if let Err(ex) = c.response_error(e) {
                    let exv = serde_json::to_value(&ex).ok();
                    log::error("Failed to send error", exv);
                }
            }
            return Ok(c.into_response());
        }
    }

    for handler in &segment.handlers {
        if let Err(e) = handler(&mut c).await {
            if let Err(ex) = c.response_error(e) {
                let exv = serde_json::to_value(&ex).ok();
                log::error("Failed to send error", exv);
            }
            break;
        }
    }

    Ok(c.into_response())
}
