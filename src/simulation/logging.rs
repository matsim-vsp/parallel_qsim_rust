use std::io;
use std::path::PathBuf;

use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

use crate::simulation::config::{Config, Profiling};
use crate::simulation::profiling::{SpanDurationToCSVLayer, WriterGuard};

pub fn init_std_out_logging() {
    let collector = tracing_subscriber::registry().with(
        fmt::Layer::new()
            .with_writer(io::stdout)
            .with_filter(LevelFilter::INFO),
    );
    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
}

pub fn init_logging(
    config: &Config,
    file_discriminant: &str,
) -> (WorkerGuard, Option<WriterGuard>) {
    let dir = PathBuf::from(&config.output().output_dir);
    let log_file_name = format!("log_process_{file_discriminant}.txt");
    let log_file_appender = rolling::never(&dir, log_file_name);
    let (log_file, _guard_log) = non_blocking(log_file_appender);

    let (csv_layer, guard) = if config.output().profiling.eq(&Profiling::CSV) {
        let duration_dir = dir.join("instrument");
        let duration_file_name = format!("instrument_process_{file_discriminant}.csv");
        let duration_path = duration_dir.join(duration_file_name);
        let (layer, writer_guard) = SpanDurationToCSVLayer::new(&duration_path);
        (Some(layer), Some(writer_guard))
    } else {
        (None, None)
    };

    let collector = tracing_subscriber::registry()
        .with(csv_layer)
        .with(
            fmt::Layer::new()
                .with_writer(io::stdout)
                .with_span_events(FmtSpan::CLOSE)
                .with_filter(LevelFilter::INFO),
        )
        .with(
            fmt::Layer::new()
                .with_writer(log_file)
                .json()
                .with_ansi(false)
                .with_filter(LevelFilter::DEBUG),
        );
    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
    (_guard_log, guard)
}
