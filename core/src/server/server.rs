use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::OnceLock;

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use rustls::ServerConfig;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_rustls::TlsAcceptor;

use crate::server::handler::handler;
use crate::server::router::Router;

// ─── Router singleton ──────────────────────────────────────────

static ROUTER: OnceLock<Router> = OnceLock::new();

pub fn set_router(router: Router) {
    let _ = ROUTER.set(router);
}

pub fn get_router() -> &'static Router {
    ROUTER
        .get()
        .expect("router no inicializado — llama set_router() antes de listen()")
}

// ─── Server ────────────────────────────────────────────────────

pub struct Server {
    addr: SocketAddr,
    shutdown_sender: Option<oneshot::Sender<()>>,
    https: bool,
    auto_cert: bool,
}

impl Server {
    pub fn new(host: &str, port: u16) -> Self {
        let addr = format!("{}:{}", host, port).parse().expect("dirección inválida");

        Self {
            addr,
            shutdown_sender: None,
            https: false,
            auto_cert: false,
        }
    }

    pub fn router(router: Router) -> &'static Router {
        set_router(router);
        get_router()
    }

    pub fn enable_https(&mut self) -> &mut Self {
        self.https = true;
        self
    }

    pub fn disable_https(&mut self) -> &mut Self {
        self.https = false;
        self
    }

    pub fn enable_auto_cert(&mut self) -> &mut Self {
        self.auto_cert = true;
        self
    }

    pub fn disable_auto_cert(&mut self) -> &mut Self {
        self.auto_cert = false;
        self
    }

    pub async fn listen(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.addr).await?;
        let (tx, mut rx) = oneshot::channel::<()>();
        self.shutdown_sender = Some(tx);

        if self.https {
            println!("forge corriendo en https://{}", self.addr);

            let tls_acceptor = if self.auto_cert {
                println!("certificado autofirmado generado");
                build_tls_acceptor()?
            } else {
                get_tls_acceptor()?
            };

            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let (stream, _) = result?;
                        let acceptor = tls_acceptor.clone();

                        tokio::spawn(async move {
                            let tls_stream = match acceptor.accept(stream).await {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!("error TLS: {:?}", e);
                                    return;
                                }
                            };

                            let io = TokioIo::new(tls_stream);

                            if let Err(e) = auto::Builder::new(TokioExecutor::new())
                                .serve_connection_with_upgrades(io, service_fn(handler))
                                .await
                            {
                                eprintln!("error en conexión HTTPS: {:?}", e);
                            }
                        });
                    }

                    _ = &mut rx => {
                        println!("server {} detenido", self.addr);
                        break;
                    }

                    _ = tokio::signal::ctrl_c() => {
                        println!("server {} detenido por Ctrl+C", self.addr);
                        break;
                    }
                }
            }
        } else {
            println!("forge corriendo en http://{}", self.addr);

            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let (stream, _) = result?;
                        let io = TokioIo::new(stream);

                        tokio::spawn(async move {
                            if let Err(e) = http1::Builder::new()
                                .keep_alive(true)
                                .serve_connection(io, service_fn(handler))
                                .await
                            {
                                eprintln!("error en conexión HTTP: {:?}", e);
                            }
                        });
                    }

                    _ = &mut rx => {
                        println!("server {} detenido", self.addr);
                        break;
                    }

                    _ = tokio::signal::ctrl_c() => {
                        println!("server {} detenido por Ctrl+C", self.addr);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_sender.take() {
            let _ = tx.send(());
        }
    }
}

// ─── TLS ───────────────────────────────────────────────────────

fn build_tls_acceptor() -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;

    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(cert.key_pair.serialize_der())?;

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(TlsAcceptor::from(std::sync::Arc::new(config)))
}

fn get_tls_acceptor() -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    let cert_bytes = std::fs::read("./tmp/certs/cert.pem")?;
    let key_bytes = std::fs::read("./tmp/certs/key.pem")?;

    let cert_der = rustls_pemfile::certs(&mut cert_bytes.as_slice())
        .next()
        .ok_or("certificado no encontrado")??;

    let key_der = rustls_pemfile::private_key(&mut key_bytes.as_slice())?.ok_or("llave privada no encontrada")?;

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(TlsAcceptor::from(std::sync::Arc::new(config)))
}
