use crate::error::HttpError;
use crate::server::router::Param;
use futures::channel::mpsc::TryRecvError::Empty;
use http_body_util::{BodyExt, Full, StreamBody, combinators::BoxBody};
use hyper::body::{Bytes, Frame, Incoming};
use hyper::{Response, header, http};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

// === Tipo de body unificado ====================================

pub type BoxBodyBytes = BoxBody<Bytes, HttpError>;

// === Modo de respuesta interno =================================

enum ResponseMode {
    /// Respuesta completa en memoria (JSON, MessagePack, HTML)
    Buffered(Response<Full<Bytes>>),

    /// Streaming: el controller envía chunks via tx, el body los lee via rx
    Streaming {
        head: http::response::Builder,
        tx: mpsc::Sender<Result<Frame<Bytes>, HttpError>>,
        rx: Option<mpsc::Receiver<Result<Frame<Bytes>, HttpError>>>,
    },
}

// === Context ===================================================

pub struct Context {
    pub req: hyper::Request<Incoming>,
    pub res: Option<ResponseMode>,
    pub params: Vec<Param>,
}

impl Context {
    pub fn new(req: hyper::Request<Incoming>, params: Vec<Param>) -> Self {
        Self { req, params, res: None }
    }

    // === Acceso rápido al request ==============================

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

    // === Query params ==========================================

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

    // === Respuestas buffered ===================================

    /// Responde en MessagePack (formato por defecto de la API)
    pub fn response<T: serde::Serialize>(&mut self, status: u16, data: &T) -> Result<(), HttpError> {
        let bytes = rmp_serde::to_vec_named(data)
            .unwrap_or_else(|e| format!(r#"{{"error":"Msgpack error: {}"}}"#, e).into_bytes());

        self.res = Some(ResponseMode::Buffered(
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/msgpack")
                .body(Full::new(Bytes::from(bytes)))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
        ));

        Ok(())
    }

    /// Responde en JSON
    pub fn response_json<T: serde::Serialize>(&mut self, status: u16, data: &T) -> Result<(), HttpError> {
        let bytes =
            serde_json::to_vec(data).unwrap_or_else(|e| format!(r#"{{"error":"JSON error: {}"}}"#, e).into_bytes());

        self.res = Some(ResponseMode::Buffered(
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(bytes)))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
        ));

        Ok(())
    }

    /// Responde con HTML
    pub fn render(&mut self, html: &str) -> Result<(), HttpError> {
        self.res = Some(ResponseMode::Buffered(
            Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Full::new(Bytes::from(html.to_owned())))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
        ));

        Ok(())
    }

    /// Status + body personalizado
    pub fn reply(&mut self, status: u16, body: impl Into<Bytes>) -> Result<(), HttpError> {
        self.res = Some(ResponseMode::Buffered(
            Response::builder()
                .status(status)
                .body(Full::new(body.into()))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))),
        ));
        Ok(())
    }

    // === Streaming =============================================

    /// Inicializa el modo streaming. Debe llamarse antes de send().
    ///
    /// ```rust
    /// ctx.stream(200, "text/plain; charset=utf-8")?;
    /// ctx.send("hola\n").await?;
    /// ctx.send("mundo\n").await?;
    /// ctx.done()
    /// ```
    pub fn stream(&mut self, status: u16, content_type: &str) -> Result<(), HttpError> {
        let (tx, rx) = mpsc::channel::<Result<Frame<Bytes>, HttpError>>(32);

        self.res = Some(ResponseMode::Streaming {
            head: Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::TRANSFER_ENCODING, "chunked"),
            tx,
            rx: Some(rx),
        });

        Ok(())
    }

    /// Envía un chunk al cliente. Requiere haber llamado stream() antes.
    pub async fn send(&self, chunk: impl Into<Bytes>) -> Result<(), HttpError> {
        match &self.res {
            Some(ResponseMode::Streaming { tx, .. }) => tx
                .send(Ok(Frame::data(chunk.into())))
                .await
                .map_err(|_| HttpError::internal_server_error("stream channel closed")),
            _ => Err(HttpError::internal_server_error(
                "send() llamado sin inicializar stream()",
            )),
        }
    }

    /// Finaliza el stream. Cierra el canal — el cliente recibe EOF.
    ///
    /// No es necesario en rigor (el canal se cierra al hacer drop del Context),
    /// pero llamarlo explícitamente deja claro el fin del handler.
    pub fn done(&mut self) -> Result<(), HttpError> {
        if let Some(ResponseMode::Streaming { tx, .. }) = &self.res {
            // Soltar el tx cierra el canal; ReceiverStream termina naturalmente
            let _ = tx; // no-op explícito para documentar la intención
        }
        Ok(())
    }

    // === Interno ===============================================

    /// Consume el Context y devuelve la respuesta lista para hyper.
    /// Usado por run_route en el server handler.
    pub fn into_response(mut self) -> Response<BoxBodyBytes> {
        match self.res.take() {
            Some(ResponseMode::Buffered(res)) => res.map(|b| b.map_err(|_| -> HttpError { unreachable!() }).boxed()),

            Some(ResponseMode::Streaming { head, rx, .. }) => {
                let rx = rx.expect("rx consumido antes de into_response()");
                let stream = ReceiverStream::new(rx);
                let body = StreamBody::new(stream).boxed();
                head.body(body).unwrap_or_else(|_| {
                    Response::builder()
                        .status(500)
                        .body(
                            Full::new(Bytes::new())
                                .map_err(|_| -> HttpError { unreachable!() })
                                .boxed(),
                        )
                        .unwrap()
                })
            }

            None => Response::builder()
                .status(500)
                .body(
                    Full::new(Bytes::from("No response set"))
                        .map_err(|_| -> HttpError { unreachable!() })
                        .boxed(),
                )
                .unwrap(),
        }
    }
}
