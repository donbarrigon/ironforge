use std::convert::Infallible;

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

pub async fn dispatch(_req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("hola mundo"))))
}
