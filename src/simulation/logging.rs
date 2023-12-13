use std::io;
use std::path::Path;

use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

pub fn init_std_out_logging() {
    let collector = tracing_subscriber::registry().with(
        fmt::Layer::new()
            .with_writer(io::stdout)
            .with_filter(LevelFilter::INFO),
    );
    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
}

pub fn init_logging(dir: &Path, file_discriminant: &str) -> (WorkerGuard, WorkerGuard) {
    let log_file_name = format!("log_process_{file_discriminant}.txt");
    let log_file_appender = rolling::never(dir, &log_file_name);
    let (log_file, _guard_log) = non_blocking(log_file_appender);

    let trace_dir = dir.join("trace");
    let trace_file_name = format!("trace_process_{file_discriminant}.txt");
    let trace_file_appender = rolling::never(&trace_dir, &trace_file_name);
    let (trace_file, _guard_performance) = non_blocking(trace_file_appender);

    let collector = tracing_subscriber::registry()
        .with(
            fmt::Layer::new()
                .with_writer(io::stdout)
                .with_filter(LevelFilter::INFO),
        )
        .with(
            fmt::Layer::new()
                .with_writer(log_file)
                .json()
                .with_ansi(false)
                .with_filter(LevelFilter::DEBUG),
        )
        .with(
            fmt::Layer::new()
                .with_writer(trace_file.with_min_level(Level::TRACE))
                .json(),
        );
    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
    (_guard_log, _guard_performance)
}
