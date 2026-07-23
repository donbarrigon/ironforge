use hyper::body::Incoming;
use hyper::{Request, Response};
use std::convert::Infallible;

use crate::handler::ResBody;
use crate::handler::context::Context;

pub async fn dispatch(req: Request<Incoming>) -> Result<Response<ResBody>, Infallible> {
    let mut c = Context::new(req);

    Ok(c.w)
}
