use hyper::Request;
use hyper::body::Incoming;

pub struct Context {
    pub req: Request<Incoming>,
    pub params: Vec<(String, String)>,
}

impl Context {
    pub fn new(req: Request<Incoming>, params: Vec<(String, String)>) -> Self {
        Self { req, params }
    }
}
