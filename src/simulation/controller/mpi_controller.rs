use crate::simulation::config::{CommandLineArgs, Config};
use crate::simulation::messaging::communication::mpi_communicator::MpiSimCommunicator;
use crate::simulation::messaging::communication::SimCommunicator;
use crate::simulation::{controller, logging};
use clap::Parser;
use mpi::collective::CommunicatorCollectives;
use mpi::topology::Communicator;
use tracing::info;

pub fn run_mpi() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let size = world.size();
    let rank = world.rank();

    let comm = MpiSimCommunicator::new(world);

    let mut args = CommandLineArgs::parse();
    // override the num part argument, with the number of processes mpi has started.
    args.num_parts = Some(size as u32);
    let config = Config::from_file(&args);

    let _guards = logging::init_logging(&config, &args.config_path, comm.rank());

    info!(
        "Starting MPI Simulation with {} partitions",
        config.partitioning().num_parts
    );
    controller::execute_partition(comm, &args);

    info!("#{} at barrier.", rank);
    universe.world().barrier();
    info!("Process #{} finishing.", rank);
}
