use std::io;
use std::path::Path;

use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

use crate::simulation::config::{Config, Logging, Profiling};
use crate::simulation::io::resolve_path;
use crate::simulation::profiling::{SpanDurationToCSVLayer, WriterGuard};

pub fn init_std_out_logging() {
    let collector = tracing_subscriber::registry().with(
        fmt::Layer::new()
            .with_writer(io::stdout)
            .with_filter(LevelFilter::INFO),
    );
    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
}

pub fn init_logging(config: &Config, part: u32) -> (Option<WorkerGuard>, Option<WriterGuard>) {
    let file_discriminant = part.to_string();
    let dir = resolve_path(config.context(), &config.output().output_dir);

    let (csv_layer, guard) = init_tracing(config, part, &file_discriminant, &dir);
    let (log_layer, log_guard) = if Logging::Info == config.output().logging {
        let log_file_name = format!("log_process_{file_discriminant}.txt");
        let log_file_appender = rolling::never(&dir, log_file_name);
        let (log_file, log_guard) = non_blocking(log_file_appender);
        let layer = fmt::Layer::new()
            .with_writer(log_file)
            .json()
            .with_ansi(false)
            .with_filter(LevelFilter::INFO);
        (Some(layer), Some(log_guard))
    } else {
        (None, None)
    };

    let collector = tracing_subscriber::registry()
        .with(csv_layer)
        .with(log_layer)
        // process 0 should log to console as well
        .with((part == 0).then(|| {
            fmt::layer()
                .with_writer(io::stdout)
                .with_span_events(FmtSpan::CLOSE)
                .with_filter(LevelFilter::INFO)
        }));

    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
    (log_guard, guard)
}

fn init_tracing(
    config: &Config,
    part: u32,
    file_discriminant: &String,
    dir: &Path,
) -> (Option<SpanDurationToCSVLayer>, Option<WriterGuard>) {
    // if we set profiling at all and if profiling is set to level trace, then each process creates an instrumenting file
    // if profiling level is set to INFO, only process 0 creates an instrument files. This is important if we run on a lot of
    // processes, because then we spent a lot of computing time on creating instrument files for each process.
    let (csv_layer, guard) = if let Profiling::CSV(level_string) = config.output().profiling {
        let level = level_string.create_tracing_level();
        if level.eq(&Level::INFO) && part == 0 || level.eq(&Level::TRACE) {
            let duration_dir = dir.join("instrument");
            let duration_file_name = format!("instrument_process_{file_discriminant}.csv");
            let duration_path = duration_dir.join(duration_file_name);
            let (layer, writer_guard) = SpanDurationToCSVLayer::new(&duration_path, level);
            (Some(layer), Some(writer_guard))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    (csv_layer, guard)
}
