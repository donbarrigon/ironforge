use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use rustls::ServerConfig;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_rustls::TlsAcceptor;

use crate::config::env;
use crate::server::Router;
use crate::server::handler::handler;

pub struct Server {
    addr: SocketAddr,
    shutdown_sender: Option<oneshot::Sender<()>>,
    https: bool,
    auto_cert: bool,
    router: Router,
}

impl Server {
    pub fn new(host: &str, port: u16, router: Router) -> Self {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let addr = format!("{}:{}", host, port).parse().expect("dirección inválida");

        return Self {
            addr,
            shutdown_sender: None,
            https: false,
            auto_cert: false,
            router,
        };
    }

    pub fn enable_https(&mut self) -> &mut Self {
        self.https = true;
        return self;
    }

    pub fn disable_https(&mut self) -> &mut Self {
        self.https = false;
        return self;
    }

    pub fn enable_auto_cert(&mut self) -> &mut Self {
        self.auto_cert = true;
        return self;
    }

    pub fn disable_auto_cert(&mut self) -> &mut Self {
        self.auto_cert = false;
        return self;
    }

    pub async fn listen(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.addr).await?;
        let (tx, mut rx) = oneshot::channel::<()>();
        self.shutdown_sender = Some(tx);

        let handler = self.handler();

        if self.https {
            // ─── HTTPS — HTTP/2 con TLS ──────────────────────────────────────
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
                        let acceptor    = tls_acceptor.clone();
                        let handler     = handler.clone();

                        tokio::spawn(async move {
                            let tls_stream = match acceptor.accept(stream).await {
                                Ok(s)  => s,
                                Err(e) => {
                                    eprintln!("error TLS: {:?}", e);
                                    return;
                                }
                            };

                            let io = TokioIo::new(tls_stream);

                            // serve_connection_with_upgrades mantiene la conexión abierta
                            if let Err(e) = auto::Builder::new(TokioExecutor::new())
                                .serve_connection_with_upgrades(io, service_fn(handler))
                                .await
                            {
                                eprintln!("error en conexión HTTPS: {:?}", e);
                            }
                        });
                    }

                    _ = &mut rx => {
                        println!("servidor {} detenido", self.addr);
                        break;
                    }

                    _ = tokio::signal::ctrl_c() => {
                        println!("servidor {} detenido por Ctrl+C", self.addr);
                        break;
                    }
                }
            }
        } else {
            // ─── HTTP — HTTP/1.1 sin TLS ─────────────────────────────────────
            println!("forge corriendo en http://{}", self.addr);

            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let (stream, _) = result?;
                        let io          = TokioIo::new(stream);
                        let handler     = handler.clone();

                        tokio::spawn(async move {
                            // keep_alive(true) mantiene la conexión abierta en HTTP/1.1
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
                        println!("servidor {} detenido", self.addr);
                        break;
                    }

                    _ = tokio::signal::ctrl_c() => {
                        println!("servidor {} detenido por Ctrl+C", self.addr);
                        break;
                    }
                }
            }
        }

        return Ok(());
    }

    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_sender.take() {
            let _ = tx.send(());
        }
    }

    fn handler(
        &self,
    ) -> impl Fn(
        Request<hyper::body::Incoming>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Response<Full<Bytes>>, Infallible>> + Send>,
    > + Clone
    + Send
    + 'static {
        let router = Arc::new(self.router.clone());

        move |req| {
            let router = Arc::clone(&router);
            Box::pin(async move { handler(router, req).await })
        }
    }
}

// genera un certificado autofirmado para desarrollo
fn build_tls_acceptor() -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    // genera certificado autofirmado con rcgen
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;

    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(cert.key_pair.serialize_der())?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)?;

    // ALPN — negocia h2 (HTTP/2) o http/1.1 automáticamente
    let mut config = config;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    return Ok(TlsAcceptor::from(Arc::new(config)));
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

    return Ok(TlsAcceptor::from(Arc::new(config)));
}

pub async fn server_start() -> Result<(), Box<dyn std::error::Error>> {
    crate::config::init()?;

    let config = env();
    let mut handles = vec![];

    for server_env in &config.server {
        let mut server = Server::new(&server_env.host, server_env.port, Router::new("test"));

        if server_env.https {
            server.enable_https();
        }

        if server_env.auto_cert {
            server.enable_auto_cert();
        }

        // cada servidor corre en su propia tarea
        let handle = tokio::spawn(async move {
            if let Err(e) = server.listen().await {
                eprintln!("server error: {:?}", e);
            }
        });

        handles.push(handle);
    }

    // espera a que todos terminen
    for handle in handles {
        handle.await?;
    }

    return Ok(());
}
