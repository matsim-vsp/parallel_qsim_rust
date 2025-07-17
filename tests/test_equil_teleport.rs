mod test_simulation;

use crate::test_simulation::TestExecutorBuilder;
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::id::store_to_file;
use rust_q_sim::simulation::io::proto::xml_events::XmlEventsWriter;
use rust_q_sim::simulation::messaging::events::EventsSubscriber;
use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;
use std::collections::HashMap;
use std::path::PathBuf;

fn create_resources(out_dir: &PathBuf, population: &str) {
    let input_dir = PathBuf::from("./assets/equil/");
    let net = Network::from_file_as_is(&input_dir.join("equil-network.xml"));
    let mut garage = Garage::from_file(&input_dir.join("equil-vehicles.xml"));
    let pop = Population::from_file(&input_dir.join(population), &mut garage);

    store_to_file(&out_dir.join("equil.ids.binpb"));
    net.to_file(&out_dir.join("equil.network.binpb"));
    pop.to_file(&out_dir.join("equil.population.binpb"));
    garage.to_file(&out_dir.join("equil.vehicles.binpb"));
}

// one agent having a network route, car being not a main mode => simulation should teleport the agent
#[test]
fn teleport_network_route() {
    let test_dir = PathBuf::from("./test_output/simulation/output-teleport-network-route/");
    create_resources(&test_dir, "equil-1-plan-network.xml");

    let config_args = CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-teleport-network-route.yml",
    );

    let mut subscribers: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>> = HashMap::new();
    subscribers.insert(0, vec![Box::new(XmlEventsWriter::new("test.xml".as_ref()))]);

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events(None)
        .additional_subscribers(subscribers)
        .build()
        .unwrap()
        .execute();
}

// one agent having a generic route, car being not a main mode => simulation should teleport the agent
#[test]
fn teleport_generic_route() {
    let test_dir = PathBuf::from("./test_output/simulation/output-teleport-generic-route/");
    create_resources(&test_dir, "equil-1-plan-generic.xml");

    let config_args = CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-teleport-generic-route.yml",
    );

    let mut subscribers: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>> = HashMap::new();
    subscribers.insert(0, vec![Box::new(XmlEventsWriter::new("test.xml".as_ref()))]);

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events(None)
        .additional_subscribers(subscribers)
        .build()
        .unwrap()
        .execute();
}

// one agent having a network route, car being a main mode => already implemented
#[test]
fn simulate_network_route() {
    let test_dir = PathBuf::from("./test_output/simulation/output-simulate-network-route/");
    create_resources(&test_dir, "equil-1-plan-network.xml");

    let config_args = CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-simulate-network-route.yml",
    );

    let mut subscribers: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>> = HashMap::new();
    subscribers.insert(0, vec![Box::new(XmlEventsWriter::new("test.xml".as_ref()))]);

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events(None)
        .additional_subscribers(subscribers)
        .build()
        .unwrap()
        .execute();
}

// one agent having a generic route, car being a main mode => simulation should crash
#[test]
#[should_panic]
fn simulate_generic_route_panics() {
    let test_dir = PathBuf::from("./test_output/simulation/output-simulate-generic-route-panics/");
    create_resources(&test_dir, "equil-1-plan-generic.xml");

    let config_args = CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-simulate-generic-route-panics.yml",
    );

    let mut subscribers: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>> = HashMap::new();
    subscribers.insert(0, vec![Box::new(XmlEventsWriter::new("test.xml".as_ref()))]);

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events(None)
        .additional_subscribers(subscribers)
        .build()
        .unwrap()
        .execute();
}
