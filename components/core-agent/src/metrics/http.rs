// External crates
use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    body::Incoming,
    header::CONTENT_TYPE,
    http::{Method, Request, Response, StatusCode},
    service::service_fn,
};
use hyper_util::{rt::TokioExecutor, server::conn::auto::Builder as HyperServerBuilder};
use prometheus::{Encoder, TextEncoder};
use std::{convert::Infallible, net::SocketAddr};
use tokio::net::TcpListener;
use tracing::instrument;

#[instrument(
    name = "metrics_server::handler",
    target = "metrics::http",
    skip_all,
    level = "debug"
)]
async fn metrics_handler(_req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    tracing::debug!("Collecting all registered prometheus metrics");
    // Gather all registered metrics
    let metrics_families = prometheus::gather();

    // Encode into Prometheus text format
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&metrics_families, &mut buffer).unwrap();
    tracing::debug!(
        encoder = ?encoder,
        writer_buffer = %buffer.len(),
        "Encoding collected metrics into prometheus text format"
    );

    let content_type = encoder.format_type().to_string();

    tracing::debug!("Building HTTP response for /metrics endpoint");
    Ok(Response::builder()
        .header(CONTENT_TYPE, content_type)
        .body(Full::new(Bytes::from(buffer)))
        .unwrap())
}

#[instrument(
    name = "metrics_server::start_metrics_server",
    target = "metrics::http",
    skip_all,
    level = "debug"
)]
pub async fn start_metrics_server(addr: &str) {
    // Parse the address and bind manually (Hyper 1.0 no longer does this automatically)
    let addr: SocketAddr = addr.parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();

    tracing::debug!(
        metrics_endpoint = %addr,
        "Core Agent performance metrics available at http://{addr}/metrics"
    );

    loop {
        let (stream, _) = listener.accept().await.unwrap();

        let io = hyper_util::rt::TokioIo::new(stream);
        let service = service_fn(|req: Request<Incoming>| async move {
            match (req.method(), req.uri().path()) {
                (&Method::GET, "/metrics") => metrics_handler(req).await,
                _ => {
                    let not_found = Full::new(Bytes::from_static(b"Not Found"));
                    Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(not_found)
                        .unwrap())
                }
            }
        });

        tracing::debug!("Spawning background task to handle connection to metrics server");
        // Spawn a task to handle the connection
        tokio::spawn(async move {
            if let Err(err) = HyperServerBuilder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await
            {
                tracing::error!(
                    error = %err,
                    "Metrics server error"
                );
            }
        });
    }
}
