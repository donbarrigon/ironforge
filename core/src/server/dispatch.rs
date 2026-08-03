use hyper::body::Incoming;
use hyper::{Request, Response};
use serde::Serialize;
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
    let k = format!("{}:{}", url, method);
    if let Some(segment) = router.static_routes.get(&k) {
        for handler in &segment.handlers {
            if let Err(e) = handler(&mut c).await {
                if let Err(ex) = c.response_error(e) {
                    let exv = serde_json::to_value(&ex).ok();
                    log::error("Failed to send error", exv);
                }
                break;
            }
        }
        // TODO: juntar en Segment middlewares y controllers en un solo campo llamado handlers
    }

    Ok(c.into_response())
}
