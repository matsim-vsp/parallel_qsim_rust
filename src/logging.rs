use flexi_logger::{detailed_format, Duplicate, FileSpec, Logger, LoggerHandle, WriteMode};
use log::{info, warn};

pub fn init_logging(directory: &str) -> Option<LoggerHandle> {
    let result = Logger::try_with_str("info")
        .unwrap()
        .log_to_file(
            FileSpec::default()
                .suppress_timestamp()
                .directory(directory),
        )
        .format(detailed_format)
        .write_mode(WriteMode::Async)
        .duplicate_to_stdout(Duplicate::All)
        .start();

    match result {
        Ok(logger_handle) => {
            info!("Created logger");
            Some(logger_handle)
        }
        Err(_) => {
            warn!("Was not able to create logger.");
            None
        }
    }
}
