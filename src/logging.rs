use flexi_logger::{detailed_format, Duplicate, FileSpec, Logger, LoggerHandle, WriteMode};

pub fn init_logging(directory: &str) -> LoggerHandle {
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(
            FileSpec::default()
                .suppress_timestamp()
                .directory(directory),
        )
        .format(detailed_format)
        .write_mode(WriteMode::Async)
        .duplicate_to_stdout(Duplicate::All)
        .start()
        .unwrap()
}
