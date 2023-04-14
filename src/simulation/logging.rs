use std::io;
use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_appender::rolling;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

pub fn init_logging(directory: &str, file_discriminant: String) {
    let mut file_name = String::from("mpi_qsim_");
    file_name.push_str(file_discriminant.as_str());

    let log_file = rolling::never(directory, &file_name);

    let mut perf_directory = String::from(directory);
    perf_directory.push_str("/trace");
    let perf_file = rolling::never(perf_directory, &file_name).with_min_level(Level::TRACE);

    let collector = tracing_subscriber::registry()
        .with(fmt::Layer::new().with_writer(log_file).with_ansi(false))
        .with(fmt::Layer::new().with_writer(perf_file).json())
        .with(
            fmt::Layer::new()
                .with_writer(io::stdout)
                .with_filter(LevelFilter::INFO),
        );
    tracing::subscriber::set_global_default(collector).expect("Unable to set a global collector");
}
