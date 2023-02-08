use clap::Parser;
use log::info;
use mpi::traits::Communicator;
use rust_q_sim::simulation::config::Config;
use rust_q_sim::simulation::logging;

fn main() {
    let universe = mpi::initialize().unwrap();
    let rank = universe.world().rank();
    let config = Config::parse();
    let _logger_handle = logging::init_logging(config.output_dir.as_ref(), Some(rank.to_string()));

    info!("{}", mpi::environment::library_version().unwrap());

    rust_q_sim::simulation::controller::run(universe.world(), config);
}
