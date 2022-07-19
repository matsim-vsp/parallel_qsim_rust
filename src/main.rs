mod logging;

use log::info;
use rust_q_sim::config::Config;
use rust_q_sim::controller;
use std::env;

fn main() {
    let config = Config::from_args(env::args());
    let _logger_handle = logging::init_logging(config.output_dir.as_ref());
    info!("Logger and Config loaded");
    info!("Starting Controller");
    controller::run(config);

    info!("Controller finished. Exiting application.")
}
