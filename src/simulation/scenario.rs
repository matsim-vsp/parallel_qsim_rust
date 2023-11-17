use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use prost::Message;
use tracing::info;

use crate::simulation::config::Config;
use crate::simulation::id;
use crate::simulation::network::global_network::Network;
use crate::simulation::population::population::Population;
use crate::simulation::vehicles::garage::Garage;

pub struct Scenario {
    network: Network,
    population: Population,
    garage: Garage,
}

fn load_scenario(config: &Config) -> Scenario {
    let network = Network::from_file(
        &config.network_file,
        config.num_parts,
        config.partition_method,
    );
    let mut garage = Garage::from_file(&PathBuf::from(&config.vehicles_file));
    let population = Population::from_file(&PathBuf::from(&config.population_file), &mut garage);
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
    id::store_to_file(&PathBuf::from(
        "/Users/janek/Desktop/3-links-output/ids.pbf",
    ));

    scenario.network.to_file(&PathBuf::from(
        "/Users/janek/Desktop/3-links-output/network.binpb",
    ));
    scenario.population.to_file(&PathBuf::from(
        "/Users/janek/Desktop/3-links-output/population.binpb",
    ));
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::Config;
    use crate::simulation::logging::init_std_out_logging;
    use crate::simulation::scenario::{load_scenario, write_scenario};

    #[test]
    #[ignore]
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
    #[ignore]
    fn load_binary() {
        init_std_out_logging();
        super::load_binary();
    }
}
