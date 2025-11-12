use std::io;
use std::path::Path;
use tracing::dispatcher::DefaultGuard;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;
use tracing_subscriber::{fmt, registry};

use crate::simulation::config::{Config, Logging, Profiling};
use crate::simulation::io::resolve_path;
use crate::simulation::profiling::routing::RoutingSpanDurationToCSVLayer;
use crate::simulation::profiling::{SpanDurationToCSVLayer, WriterGuard};

// This is a helper struct to store the logger guards. When they are dropped, logging can be reset.
#[allow(dead_code)]
pub(crate) struct LogGuards {
    tracing_guards: (
        Option<WriterGuard>,
        Option<crate::simulation::profiling::routing::WriterGuard>,
    ),
    log_guard: Option<WorkerGuard>,
    default: DefaultGuard,
}

pub fn init_std_out_logging_thread_local() -> DefaultGuard {
    let collector = tracing_subscriber::registry().with(
        fmt::Layer::new()
            .with_writer(io::stdout)
            .with_filter(LevelFilter::INFO),
    );
    tracing::subscriber::set_default(collector)
}

pub(crate) fn init_logging(config: &Config, part: u32) -> LogGuards {
    let file_discriminant = part.to_string();
    let dir = resolve_path(config.context(), &config.output().output_dir);

    let csv_layers = init_tracing(config, part, &file_discriminant, &dir);
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

    let console_layer = (part == 0).then(|| {
        fmt::layer()
            .with_writer(io::stdout)
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(LevelFilter::INFO)
    });

    // Add `Optional`s. If None, then the corresponding layer is not added.
    let collector = registry()
        .with(log_layer)
        .with(console_layer)
        .with(csv_layers.routing)
        .with(csv_layers.general);

    let default = tracing::subscriber::set_default(collector);

    LogGuards {
        tracing_guards: (csv_layers.general_guard, csv_layers.routing_guard),
        log_guard,
        default,
    }
}

fn init_tracing(config: &Config, part: u32, file_discriminant: &String, dir: &Path) -> CsvLayers {
    // if we set profiling at all and if profiling is set to level trace, then each process creates an instrumenting file
    // if profiling level is set to INFO, only process 0 creates an instrument file. This is important if we run on a lot of
    // processes, because then we spent a lot of computing time on creating instrument files for each process.
    if let Profiling::CSV(level_string) = config.output().profiling {
        let level = level_string.create_tracing_level();
        if level.eq(&Level::INFO) && part == 0 || level.eq(&Level::TRACE) {
            let instrument_dir = dir.join("instrument");
            let duration_file_name = format!("instrument_process_{file_discriminant}.csv");
            let duration_path = instrument_dir.join(duration_file_name);
            let (general, general_guard) = SpanDurationToCSVLayer::new(&duration_path, level);

            let routing_file_name = format!("routing_process_{file_discriminant}.csv");
            let routing_path = instrument_dir.join(routing_file_name);
            let (routing, routing_guard) =
                RoutingSpanDurationToCSVLayer::new(&routing_path, level, "rust_qsim");

            CsvLayers {
                general: Some(general),
                general_guard: Some(general_guard),
                routing: Some(routing),
                routing_guard: Some(routing_guard),
            }
        } else {
            CsvLayers::new()
        }
    } else {
        CsvLayers::new()
    }
}

#[derive(Default)]
struct CsvLayers {
    general: Option<SpanDurationToCSVLayer>,
    general_guard: Option<WriterGuard>,
    routing: Option<RoutingSpanDurationToCSVLayer>,
    routing_guard: Option<crate::simulation::profiling::routing::WriterGuard>,
}

impl CsvLayers {
    pub fn new() -> Self {
        Self {
            general: None,
            general_guard: None,
            routing: None,
            routing_guard: None,
        }
    }
}
