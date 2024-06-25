use std::io;
use std::path::PathBuf;

use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

use crate::simulation::config::{Config, Logging, Profiling};
use crate::simulation::profiling::{SpanDurationToCSVLayer, WriterGuard};

pub fn init_std_out_logging() {
    let collector = tracing_subscriber::registry().with(
        fmt::Layer::new()
            .with_writer(io::stdout)
            .with_filter(LevelFilter::INFO),
    );
    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
}

pub fn init_logging(config: &Config, part: u32) -> (WorkerGuard, Option<WriterGuard>) {
    let file_discriminant = part.to_string();
    let dir = PathBuf::from(&config.output().output_dir);
    let log_file_name = format!("log_process_{file_discriminant}.txt");
    let log_file_appender = rolling::never(&dir, log_file_name);
    let (log_file, _guard_log) = non_blocking(log_file_appender);

    let (csv_layer, guard) = if let Profiling::CSV(level) = config.output().profiling {
        let duration_dir = dir.join("instrument");
        let duration_file_name = format!("instrument_process_{file_discriminant}.csv");
        let duration_path = duration_dir.join(duration_file_name);
        let (layer, writer_guard) =
            SpanDurationToCSVLayer::new(&duration_path, level.create_tracing_level());
        (Some(layer), Some(writer_guard))
    } else {
        (None, None)
    };

    let collector = tracing_subscriber::registry()
        .with(csv_layer)
        .with((config.output().logging == Logging::Info).then(|| {
            fmt::Layer::new()
                .with_writer(log_file)
                .json()
                .with_ansi(false)
                .with_filter(LevelFilter::INFO)
        }))
        // process 0 should log to console as well
        .with((part == 0).then(|| {
            fmt::layer()
                .with_writer(io::stdout)
                .with_span_events(FmtSpan::CLOSE)
                .with_filter(LevelFilter::INFO)
        }));

    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
    (_guard_log, guard)
}
