use clap::Parser;
use log::info;
use rust_q_sim::config::Config;
use rust_q_sim::{controller, logging};

fn main() {
    let config = Config::parse();
    let _logger_handle = logging::init_logging(config.output_dir.as_ref());
    info!("Logger and Config loaded {config:?}");

    info!("Starting Controller");
    controller::run(config);

    info!("Controller finished. Exiting application.")
}
