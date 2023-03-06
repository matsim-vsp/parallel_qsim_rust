use crate::simulation::config::{Config, RoutingMode};
use crate::simulation::id_mapping::MatsimIdMappings;
use crate::simulation::io::network::IONetwork;
use crate::simulation::io::population::IOPopulation;
use crate::simulation::io::proto_events::ProtoEventsWriter;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::message_broker::MpiMessageBroker;
use crate::simulation::messaging::messages::proto::Vehicle;
use crate::simulation::messaging::travel_time_collector::TravelTimeCollector;
use crate::simulation::network::partitioned_network::Network;
use crate::simulation::partition_info::PartitionInfo;
use crate::simulation::population::Population;
use crate::simulation::routing::network_converter::NetworkConverter;
use crate::simulation::routing::router::Router;
use crate::simulation::routing::rust_road_router_adapter::RustRoadRouterAdapter;
use crate::simulation::simulation::Simulation;
use log::info;
use mpi::topology::SystemCommunicator;
use mpi::traits::{Communicator, CommunicatorCollectives};
use std::ffi::c_int;
use std::fs;
use std::fs::remove_dir_all;
use std::ops::Sub;
use std::path::PathBuf;
use std::time::Instant;

pub fn run(world: SystemCommunicator, config: Config) {
    let rank = world.rank();
    let size = world.size();

    info!("Process #{rank} of {size}");

    let output_path = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    let io_network = IONetwork::from_file(config.network_file.as_ref());
    let io_population = IOPopulation::from_file(config.population_file.as_ref());
    let id_mappings = MatsimIdMappings::from_io(&io_network, &io_population);
    let partition_info = PartitionInfo::from_io_network(&io_network, &id_mappings, size as usize);
    let mut network: Network<Vehicle> = Network::from_io(
        &io_network,
        size as usize,
        config.sample_size,
        |node| partition_info.get_partition(node),
        &id_mappings,
    );

    // write network with new ids to output but only once.
    if rank == 0 {
        let out_network =
            io_network.clone_with_internal_ids(&network, &id_mappings.links, &id_mappings.nodes);
        out_network.to_file(&output_path.join("output_network.xml.gz"));

        let id_mappings_string = serde_json::to_string(id_mappings.links.as_ref()).unwrap();
        fs::write(
            config.output_dir.to_owned() + "/id_mappings.json",
            id_mappings_string,
        )
        .expect("Unable to write file");
        info!("Written id mappings file!");
    }

    let routing_kit_network = NetworkConverter::convert_io_network(io_network, Some(&id_mappings));

    let population = Population::from_io(
        &io_population,
        &id_mappings,
        rank as usize,
        &network,
        config.routing_mode,
    );
    let network_partition = network.partitions.remove(rank as usize);
    info!(
        "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
        network_partition.nodes.len(),
        network_partition.links.len(),
        population.agents.len()
    );

    let neighbors = network_partition
        .neighbors()
        .iter()
        // cast this here. change the api to not use usize all the time, since with mpi and protobuf
        // we have to use u32 or u64.
        .map(|u| *u as u32)
        .collect();
    let link_id_mapping = network.links_2_partition;

    let message_broker = MpiMessageBroker::new(world, rank, neighbors, link_id_mapping);
    let mut events = EventsPublisher::new();

    let events_file = format!("events.{rank}.pbf");
    let events_path = output_path.join(events_file);
    events.add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    let travel_time_collector = Box::new(TravelTimeCollector::new());
    events.add_subscriber(travel_time_collector);
    //events.add_subscriber(Box::new(EventsLogger {}));

    let mut router: Option<Box<dyn Router>> = None;
    if config.routing_mode == RoutingMode::AdHoc {
        router = Some(Box::new(RustRoadRouterAdapter::new(
            &routing_kit_network,
            get_temp_output_folder(&config.output_dir, rank).as_str(),
        )));
    }

    let mut simulation = Simulation::new(
        &config,
        network_partition,
        population,
        message_broker,
        events,
        router,
    );

    let start = Instant::now();
    simulation.run(config.start_time, config.end_time);
    let end = Instant::now();
    let duration = end.sub(start).as_millis() / 1000;
    info!("#{rank} took: {duration}s");

    info!("output dir: {:?}", config.output_dir);

    if rank == 0 && config.routing_mode == RoutingMode::AdHoc {
        remove_dir_all(config.output_dir + "routing")
            .expect("Wasn't able to delete temporary routing output.")
    }

    info!("#{rank} at barrier.");
    world.barrier();
    info!("Process #{rank} finishing.");
}

fn get_temp_output_folder(output_dir: &str, rank: c_int) -> String {
    format!("{}{}{}{}", output_dir, "/routing/", rank, "/")
}
