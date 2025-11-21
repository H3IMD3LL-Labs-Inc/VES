use std::panic;
use tracing::error;
use tracing_appender::rolling;
use tracing_error::ErrorLayer;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::Fmt;

pub fn init_tracing() {
    let file_appender = rolling::daily("/var/log/ves", "ves_runtime.log");
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = fmt::layer()
        .with_writer(non_blocking_writer)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_timer(fmt::time::UtcTime::rfc_3339());

    let json_layer = fmt::layer()
        .json()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_timer(fmt::time::UtcTime::rfc_3339());

    let error_layer = ErrorLayer::default();

    let console_layer = console_subscriber::spawn();

    let subscriber = Registry::default()
        .with(filter)
        .with(fmt_layer)
        .with(json_layer)
        .with(console_layer)
        .with(error_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global tracing subscriber");
}

pub fn init_panic_handler() {
    panic::set_hook(Box::new(|panic_info| {
        let msg = match panic_info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => "Unknown panic",
        };

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_string());

        error!(
            message = %msg,
            location = %location,
            "Application panicked!"
        );
    }));
}
