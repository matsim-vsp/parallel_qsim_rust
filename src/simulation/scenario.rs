use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use prost::Message;
use tracing::info;

use crate::simulation::config::Config;
use crate::simulation::id;
use crate::simulation::io::population::IOPopulation;
use crate::simulation::network::global_network::Network;
use crate::simulation::population::global_population::load_population;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::population::Population;

pub struct Scenario {
    network: Network,
    population: Vec<Population>,
    garage: Garage,
}

fn load_scenario(config: &Config) -> Scenario {
    let network = Network::from_file(
        &config.network_file,
        config.num_parts,
        config.partition_method,
    );
    let mut garage = Garage::from_file(&config.vehicles_file);
    let io_pop = IOPopulation::from_file(&config.population_file);
    let population = load_population(&io_pop, &network, &mut garage, config.num_parts);
    Scenario {
        network,
        population,
        garage,
    }
}

fn load_binary() {
    let folder = PathBuf::from("/Users/janek/Desktop/3-links-output/");
    id::load_from_file(&folder.join("ids.pbf"));

    let mut reader =
        BufReader::new(File::open(&folder.join("network.pbf")).expect("Could not open file"));
    let mut net_bytes = Vec::new();

    info!("Starting to load network");
    reader
        .read_to_end(&mut net_bytes)
        .expect("Failed to read network bytes");
    let wire_net = crate::simulation::wire_types::network::Network::decode(net_bytes.as_slice())
        .expect("Failed to decode network");
    info!(
        "Network loaded. It has {} nodes and {} links",
        wire_net.nodes.len(),
        wire_net.links.len()
    );

    info!("Starting to load population");
    let mut reader =
        BufReader::new(File::open(&folder.join("population.pbf")).expect("Could not open file"));
    let mut pop_bytes = Vec::new();

    reader
        .read_to_end(&mut pop_bytes)
        .expect("Failed to read population file");
    let wire_pop =
        crate::simulation::wire_types::population::Population::decode(pop_bytes.as_slice())
            .expect("Failed to decode population");

    info!(
        "Finished reading population it contains {} persons",
        wire_pop.persons.len()
    );
}

fn write_scenario(scenario: &Scenario) {
    info!("Writing id store");
    crate::simulation::id::store_to_file(&PathBuf::from(
        "/Users/janek/Desktop/3-links-output/ids.pbf",
    ));

    info!("Converting to wire network");
    let nodes: Vec<_> = scenario
        .network
        .nodes
        .iter()
        .map(|n| crate::simulation::wire_types::network::Node {
            id: n.id.internal(),
            x: n.x,
            y: n.y,
            partition: n.partition,
        })
        .collect();
    let links: Vec<_> = scenario
        .network
        .links
        .iter()
        .map(|l| crate::simulation::wire_types::network::Link {
            id: l.id.internal(),
            from: l.from.internal(),
            to: l.to.internal(),
            length: l.length,
            capacity: l.capacity,
            freespeed: l.freespeed,
            permlanes: l.permlanes,
            modes: l.modes.iter().map(|id| id.internal()).collect(),
            partition: l.partition,
        })
        .collect();
    let wire_network = crate::simulation::wire_types::network::Network { nodes, links };

    info!("Encoding network");
    let net_bytes = wire_network.encode_to_vec();

    let mut file = File::create(PathBuf::from(
        "/Users/janek/Desktop/3-links-output/network.pbf",
    ))
    .expect("failed to create file");
    info!("Writing bytes to file");
    file.write_all(&net_bytes)
        .expect("Failed to write to network file");

    info!("converting population");
    let global_population = scenario
        .population
        .iter()
        .flat_map(|pop| pop.persons.iter())
        .fold(
            crate::simulation::wire_types::population::Population { persons: vec![] },
            |mut pop, person| {
                pop.persons.push(person.clone());
                pop
            },
        );

    info!("Encoding population");
    let pop_bytes = global_population.encode_to_vec();
    let mut file = File::create(PathBuf::from(
        "/Users/janek/Desktop/3-links-output/population.pbf",
    ))
    .expect("failed to create file");
    info!("Writing population bytes to file");
    file.write_all(&pop_bytes)
        .expect("Failed to write to population file");
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::Config;
    use crate::simulation::logging::init_std_out_logging;
    use crate::simulation::scenario::{load_scenario, write_scenario};

    #[test]
    fn load_write() {
        init_std_out_logging();
        let config = Config::builder()
            .network_file("/Users/janek/Documents/rust_q_sim/input/rvr.network.xml.gz".to_owned())
            .population_file(
                "/Users/janek/Documents/rust_q_sim/input/rvr-10pct.plans.xml.gz".to_owned(),
            )
            .vehicles_file("/Users/janek/Documents/rust_q_sim/input/rvr.vehicles.xml".to_owned())
            .build();

        let scenario = load_scenario(&config);
        write_scenario(&scenario);
    }

    #[test]
    fn load_binary() {
        init_std_out_logging();
        super::load_binary();
    }
}
