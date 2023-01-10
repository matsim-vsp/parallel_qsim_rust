use clap::Parser;
use log::info;
use rust_q_sim::config::Config;
use rust_q_sim::logging;

fn main() {
    let universe = mpi::initialize().unwrap();
    let config = Config::parse();
    let _logger_handle = logging::init_logging(config.output_dir.as_ref());

    info!("{}", mpi::environment::library_version().unwrap());

    rust_q_sim::mpi::controller::run(universe.world(), config);
}
