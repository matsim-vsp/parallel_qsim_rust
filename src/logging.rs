use flexi_logger::{detailed_format, Duplicate, FileSpec, Logger, LoggerHandle, WriteMode};

pub fn init_logging(directory: &str, file_discriminant: Option<String>) -> LoggerHandle {
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(
            FileSpec::default()
                .o_discriminant(file_discriminant)
                .suppress_timestamp()
                .directory(directory),
        )
        .format(detailed_format)
        .write_mode(WriteMode::Async)
        .duplicate_to_stdout(Duplicate::All)
        .start()
        .unwrap()
}
