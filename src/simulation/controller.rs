use std::{fs, thread};
use std::ops::Sub;
use std::path::PathBuf;
use std::time::Instant;

use mpi::topology::SystemCommunicator;
use mpi::traits::{Communicator, CommunicatorCollectives};
use tracing::info;

use crate::simulation::config::Config;
use crate::simulation::io::proto_events::ProtoEventsWriter;
use crate::simulation::messaging::events::{EventsLogger, EventsPublisher};
use crate::simulation::messaging::message_broker;
use crate::simulation::messaging::message_broker::{
    ChannelNetCommunicator, DummyNetCommunicator, MpiNetCommunicator, NetMessageBroker,
};
use crate::simulation::network::global_network::Network;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::population::population::Population;
use crate::simulation::simulation::Simulation;
use crate::simulation::vehicles::garage::Garage;

pub fn run_single_thread(config: Config) {
    info!("Starting single threaded controller!");

    let output_path = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    let mut garage = Garage::from_file(config.vehicles_file.as_ref());

    let network = Network::from_file(config.network_file.as_ref(), 0, &mut garage);

    let population =
        Population::from_file(config.population_file.as_ref(), &network, &mut garage, 0);

    let sim_network = SimNetworkPartition::from_network(&network, 0);
    let message_broker =
        NetMessageBroker::<DummyNetCommunicator>::new_single_partition(&sim_network);

    let mut events = EventsPublisher::new();
    let events_path = output_path.join("events.pbf");
    events.add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    events.add_subscriber(Box::new(EventsLogger {}));

    let mut simulation = Simulation::new(
        &config,
        sim_network,
        garage,
        population,
        message_broker,
        events,
    );

    info!("Starting single threaded simulation");
    simulation.run(config.start_time, config.end_time);
    info!("Finished single threaded simulation");
}

pub fn run_local_multithreaded(config: Config) {
    info!("Starting single threaded controller!");

    let output_path = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    let mut garage = Garage::from_file(config.vehicles_file.as_ref());

    let network = Network::from_file(config.network_file.as_ref(), 0, &mut garage);

    let population =
        Population::from_file(config.population_file.as_ref(), &network, &mut garage, 0);

    let comms = ChannelNetCommunicator::create_n_2_n(config.num_parts);

    let handles = comms.into_iter().map(|comm| {
        let sim_network = SimNetworkPartition::from_network(&network, 0);
        let mut events = EventsPublisher::new();
        let events_file = format!("events.{}.pbf", comm.rank);
        let events_path = output_path.join("events.pbf");
        events.add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
        events.add_subscriber(Box::new(EventsLogger {}));

        let message_broker = NetMessageBroker::<MpiNetCommunicator>::new_channel_broker(comm, &sim_network);

        let mut simulation = Simulation::new(
            &config,
            sim_network,
            garage,
            population,
            message_broker,
            events,
        )
    })
        .map(|sim| thread::spawn(move || sim.run(config.start_time, config.end_time)))
        .collect();


    let sim_network = SimNetworkPartition::from_network(&network, 0);

    let mut events = EventsPublisher::new();
    let events_path = output_path.join("events.pbf");
    events.add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    events.add_subscriber(Box::new(EventsLogger {}));


    let mut simulation = Simulation::new(
        &config,
        sim_network,
        garage,
        population,
        message_broker,
        events,
    );

    info!("Starting single threaded simulation");
    simulation.run(config.start_time, config.end_time);
    info!("Finished single threaded simulation");
}

pub fn run(world: SystemCommunicator, config: Config) {
    let rank = world.rank();
    let size = world.size();

    info!("Process #{rank} of {size}");

    let output_path = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    let mut garage = Garage::from_file(config.vehicles_file.as_ref());

    let network = Network::from_file(config.network_file.as_ref(), config.num_parts, &mut garage);

    // write network with new ids to output but only once.
    if rank == 0 {
        network.to_file(&output_path.join("output_network.xml.gz"));
    }

    let population = Population::from_file(
        config.population_file.as_ref(),
        &network,
        &mut garage,
        rank as usize,
    );
    let network_partition = SimNetworkPartition::from_network(&network, rank as usize);
    info!(
        "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
        network_partition.nodes.len(),
        network_partition.links.len(),
        population.agents.len()
    );

    let message_broker =
        NetMessageBroker::<MpiNetCommunicator>::new_mpi_broker(world, &network_partition);
    let mut events = EventsPublisher::new();

    let events_file = format!("events.{rank}.pbf");
    let events_path = output_path.join(events_file);
    events.add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    // let travel_time_collector = Box::new(TravelTimeCollector::new());
    //events.add_subscriber(travel_time_collector);
    //events.add_subscriber(Box::new(EventsLogger {}));

    let mut simulation = Simulation::new(
        &config,
        network_partition,
        garage,
        population,
        message_broker,
        events,
    );

    let start = Instant::now();
    simulation.run(config.start_time, config.end_time);
    let end = Instant::now();
    let duration = end.sub(start).as_millis() / 1000;
    info!("#{rank} took: {duration}s");

    info!("output dir: {:?}", config.output_dir);

    info!("#{rank} at barrier.");
    world.barrier();
    info!("Process #{rank} finishing.");
}

#[cfg(test)]
mod test {
    use std::sync::Once;

    use tracing_appender::non_blocking::WorkerGuard;

    use crate::simulation::config::Config;
    use crate::simulation::controller::run_single_thread;
    use crate::simulation::logging;

    static INIT: Once = Once::new();
    static mut WORKER_GUARDS: Option<(WorkerGuard, WorkerGuard)> = None;

    pub fn initialize() {
        // hacky hack to initialize logger only once
        unsafe {
            INIT.call_once(|| {
                let guards =
                    logging::init_logging("./test_output/simulation/controller", String::from("0"));
                WORKER_GUARDS = Some(guards);
            });
        }
    }

    #[test]
    fn execute_3_link_example() {
        initialize();
        let config = Config::builder()
            .network_file(String::from("./assets/3-links/3-links-network.xml"))
            .population_file(String::from("./assets/3-links/1-agent-full-leg.xml"))
            .vehicles_file(String::from("./assets/3-links/vehicles.xml"))
            .output_dir(String::from(
                "./test_output/simulation/controller/execute_3_link_example",
            ))
            .build();

        //let _guards = logging::init_logging(config.output_dir.as_ref(), String::from("0"));

        run_single_thread(config);
    }

    #[test]
    fn execute_equil_example() {
        initialize();
        let config = Config::builder()
            .network_file(String::from("./assets/equil/equil-network.xml"))
            .population_file(String::from("./assets/equil/equil-plans.xml.gz"))
            .vehicles_file(String::from("./assets/equil/equil-vehicles.xml"))
            .output_dir(String::from(
                "./test_output/simulation/controller/execute_equil_example",
            ))
            .build();

        //let _guards = logging::init_logging(config.output_dir.as_ref(), String::from("0"));

        run_single_thread(config);
    }
}
