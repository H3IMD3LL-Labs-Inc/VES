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

async fn metrics_handler(_req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    // Gather all registered metrics
    let metrics_families = prometheus::gather();

    // Encode into Prometheus text format
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&metrics_families, &mut buffer).unwrap();

    let content_type = encoder.format_type().to_string();

    Ok(Response::builder()
        .header(CONTENT_TYPE, content_type)
        .body(Full::new(Bytes::from(buffer)))
        .unwrap())
}

pub async fn start_metrics_server(addr: &str) {
    // Parse the address and bind manually (Hyper 1.0 no longer does this automatically)
    let addr: SocketAddr = addr.parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();

    println!("Performance metrics available at http://{}/metrics", addr);

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

        // Spawn a task to handle the connection
        tokio::spawn(async move {
            if let Err(err) = HyperServerBuilder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await
            {
                eprintln!("Metrics server error: {err}");
            }
        });
    }
}
