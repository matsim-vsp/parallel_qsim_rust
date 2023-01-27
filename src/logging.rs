use flexi_logger::{detailed_format, Duplicate, FileSpec, Logger, LoggerHandle, WriteMode};
use log::{info, warn};

pub fn init_logging(directory: &str, file_discriminant: Option<String>) -> Option<LoggerHandle> {
    let result = Logger::try_with_str("info")
        .unwrap()
        .log_to_file(
            FileSpec::default()
                .o_discriminant(file_discriminant)
                .suppress_timestamp()
                .directory(directory),
        )
        .format(detailed_format)
        .write_mode(WriteMode::Direct)
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
