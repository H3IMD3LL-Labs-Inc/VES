use hyper::{
    Body, Method, Request, Response, Server,
    service::{make_service_fn, service_fn},
};
use prometheus::{Encoder, TextEncoder};
use std::convert::Infalliable;

async fn metrics_handler(_req: Request<Body>) -> Result<Response<Body>, Infalliable> {
    // Gather all registered metrics
    let metrics_families = prometheus::gather();

    // Encode them into the Prometheus text format
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&metrics_families, &mut buffer).unwrap();

    // Return as HTTP response
    Ok(Response::new(Body::from(buffer)))
}

pub async fn start_metrics_server(addr: &str) {
    // Build service factory
    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infalliable>(service_fn(|req: Request<Body>| async move {
            match (req.method(), req.uri().path()) {
                (&Method::GET, "/metrics") => metrics_handler(req).await,
                _ => Ok(Response::builder()
                    .status(404)
                    .body(Body::from("Not Found"))
                    .unwrap()),
            }
        }))
    });

    // Parse and bind address
    let addr = addr.parse().unwrap();
    let server = Server::bind(&addr).serve(make_svc);

    println!("Performance metrics available at http://{}/metrics", addr);

    if let Err(e) = server.await {
        eprintln!("Metrics server error: {}", e)
    }
}
