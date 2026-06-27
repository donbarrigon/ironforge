use crate::error::HttpError;
use crate::server::router::Param;
use ahash::AHashMap;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Response, body::Incoming, header};
use std::sync::Arc;

pub struct Context {
    pub req: hyper::Request<Incoming>,
    pub params: Vec<Param>,
    pub map: Arc<AHashMap<String, RouteMap>>,
    response: Option<Response<Full<Bytes>>>,
}

impl Context {
    pub fn new(req: hyper::Request<Incoming>, params: Vec<Param>, map: Arc<AHashMap<String, RouteMap>>) -> Self {
        Self {
            req,
            params,
            map,
            response: None,
        }
    }

    // ─── Métodos de acceso rápido ──────────────────────────────

    pub fn method(&self) -> &hyper::Method {
        self.req.method()
    }

    pub fn path(&self) -> &str {
        self.req.uri().path()
    }

    pub fn headers(&self) -> &hyper::HeaderMap {
        self.req.headers()
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.iter().find(|p| p.name == name).map(|p| p.value.as_str())
    }

    // ─── Query Params ──────────────────────────────────────────

    /// Parsea los query params de la URL y los devuelve como Vec<Param>
    pub fn query(&self) -> Vec<Param> {
        self.req
            .uri()
            .query()
            .map(|q| {
                q.split('&')
                    .filter_map(|pair| {
                        let mut parts = pair.splitn(2, '=');
                        let name = parts.next()?.to_string();
                        let value = parts.next().unwrap_or("").to_string();
                        Some(Param { name, value })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    // ─── Respuestas (devuelven Ok(()) para usar con ?) ─────────

    /// Responde en MessagePack (por defecto) y termina el handler
    pub fn response<T: serde::Serialize>(&mut self, status: u16, data: &T) -> Result<(), HttpError> {
        let bytes = rmp_serde::to_vec_named(data)
            .unwrap_or_else(|e| format!(r#"{{"error":"Msgpack error: {}"}}"#, e).into_bytes());

        self.response = Some(
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/msgpack")
                .body(Full::new(Bytes::from(bytes)))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
        );

        Ok(())
    }

    /// Responde en JSON y termina el handler
    pub fn response_json<T: serde::Serialize>(&mut self, status: u16, data: &T) -> Result<(), HttpError> {
        let bytes =
            serde_json::to_vec(data).unwrap_or_else(|e| format!(r#"{{"error":"JSON error: {}"}}"#, e).into_bytes());

        self.response = Some(
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(bytes)))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
        );

        Ok(())
    }

    /// Responde HTML (próximamente)
    pub fn render(&mut self, _html: &str) -> Result<(), HttpError> {
        self.response = Some(
            Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Full::new(Bytes::from("<h1>TODO: próximamente</h1>")))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
        );

        Ok(())
    }

    // ─── Streaming ─────────────────────────────────────────────

    /// Envía un chunk de datos en streaming
    pub async fn send(&mut self, _chunk: impl Into<Bytes>) -> Result<(), HttpError> {
        // TODO: implementar streaming
        Ok(())
    }

    /// Finaliza el stream y termina el handler
    pub fn done(&mut self) -> Result<(), HttpError> {
        self.response = Some(Response::new(Full::new(Bytes::new())));
        Ok(())
    }

    // ─── Interno ───────────────────────────────────────────────

    /// Consume el Context y devuelve la respuesta (usado por run_route)
    pub fn into_response(self) -> Response<Full<Bytes>> {
        self.response.unwrap_or_else(|| {
            Response::builder()
                .status(500)
                .body(Full::new(Bytes::from("No response set")))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
        })
    }
}
