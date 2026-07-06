pub mod facility;
pub mod network;
pub mod population;
pub mod prepare_for_sim;
pub mod trip_structure_utils;
pub mod vehicles;

use crate::simulation::config::Config;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::{id, io};
use network::Network;
use population::Population;
use std::sync::Arc;
use tracing::info;
use vehicles::Garage;

#[derive(Debug, Clone, PartialEq)]
pub struct Coordinate {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Coordinate {
    pub fn new_2d(x: f64, y: f64) -> Self {
        Self { x, y, z: 0. }
    }

    pub fn new_3d(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn euclidean_distance(a: &Coordinate, b: &Coordinate) -> f64 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn middle(a: &Self, b: &Self) -> Self {
        Coordinate::new_3d((a.x + b.x) / 2., (a.y + b.y) / 2., (a.z + b.z) / 2.)
    }
}

impl Default for Coordinate {
    fn default() -> Self {
        Self::new_3d(0.0, 0.0, 0.0)
    }
}

/// The scenario as it comes from input files: fully owned and still local to the loading thread.
#[derive(Debug)]
pub struct Scenario {
    pub network: Network,
    pub garage: Garage,
    pub population: Population,
    pub config: Arc<Config>,
}

impl Scenario {
    pub fn load<C: Into<Arc<Config>>>(config: C) -> Self {
        info!("Start loading mod.");

        let config = config.into();

        if let Some(path) = &config.ids().path {
            info!("Loading IDs from {:?}", path);
            id::load_from_file(&io::resolve_path(config.context(), path));
        }

        // mandatory content to create a mod
        let network = Self::load_network(&config);
        let mut garage = Self::load_garage(&config);
        let population = Self::load_population(&config, &mut garage);

        Scenario {
            network,
            garage,
            population,
            config,
        }
    }

    fn load_network(config: &Config) -> Network {
        if let Some(path) = &config.network().path {
            let net_in_path = io::resolve_path(config.context(), path);
            let num_parts = config.partitioning().num_parts;
            Network::from_file_path(&net_in_path, num_parts, &config.partitioning().method)
        } else {
            Network::default()
        }
    }

    fn load_garage(config: &Config) -> Garage {
        if let Some(path) = &config.vehicles().path {
            let garage_in_path = io::resolve_path(config.context(), path);
            Garage::from_file(&garage_in_path)
        } else {
            Garage::default()
        }
    }

    fn load_population(config: &Config, garage: &mut Garage) -> Population {
        if let Some(path) = &config.population().path {
            let pop_in_path = io::resolve_path(config.context(), path);
            Population::from_file(&pop_in_path, garage)
        } else {
            Population::default()
        }
    }
}

/// Immutable scenario data shared by controller, mobsim partitions and replanning phases.
#[derive(Debug, Clone)]
pub struct ScenarioCore {
    pub network: Arc<Network>,
    pub garage: Arc<Garage>,
    pub config: Arc<Config>,
}

/// Controller-owned scenario state between phases.
#[derive(Debug)]
pub struct ControllerScenario {
    pub core: ScenarioCore,
    pub population: Population,
}

/// Owned population fragment passed between execution phases.
#[derive(Debug, Default)]
pub struct PopulationShard {
    pub population: Population,
}

/// Static and per-run runtime context for one mobsim partition.
#[derive(Debug)]
pub struct MobsimScenarioPartition {
    pub rank: u32,
    pub scenario: ScenarioCore,
    pub network_partition: SimNetworkPartition,
}

/// Input for one mobsim partition run.
#[derive(Debug)]
pub struct MobsimInput {
    pub partition: MobsimScenarioPartition,
    pub population: PopulationShard,
}

impl From<Scenario> for ControllerScenario {
    fn from(scenario: Scenario) -> Self {
        Self {
            core: ScenarioCore {
                network: Arc::new(scenario.network),
                garage: Arc::new(scenario.garage),
                config: scenario.config,
            },
            population: scenario.population,
        }
    }
}

impl ControllerScenario {
    pub fn split_for_mobsim(&mut self) -> Vec<MobsimInput> {
        let num_parts = self.core.config.partitioning().num_parts;
        let population = std::mem::take(&mut self.population);
        population
            .split_by_start_link_partition(&self.core.network, num_parts)
            .into_iter()
            .enumerate()
            .map(|(rank, population)| self.create_mobsim_input(rank as u32, population))
            .collect()
    }

    #[cfg(test)]
    pub fn merge_population_shards(&mut self, shards: Vec<PopulationShard>) {
        for shard in shards {
            for (id, person) in shard.population.persons {
                let previous = self.population.persons.insert(id.clone(), person);
                assert!(
                    previous.is_none(),
                    "Person {id} was returned by more than one population shard"
                );
            }
        }
    }

    pub fn replace_population(&mut self, population: Population) {
        assert!(
            self.population.persons.is_empty(),
            "Controller still owns population while replacing it after a phase."
        );
        self.population = population;
    }

    fn create_mobsim_input(&self, rank: u32, population: Population) -> MobsimInput {
        let network_partition = Self::create_network_partition(&self.core, rank);

        info!(
            "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
            network_partition.nodes.len(),
            network_partition.links.len(),
            population.persons.len()
        );

        MobsimInput {
            partition: MobsimScenarioPartition {
                rank,
                // Since core holds Arcs, this clone is cheap.
                scenario: self.core.clone(),
                network_partition,
            },
            population: PopulationShard { population },
        }
    }

    fn create_network_partition(core: &ScenarioCore, rank: u32) -> SimNetworkPartition {
        let base_seed = core.config.computational_setup().random_seed;
        SimNetworkPartition::from_network(&core.network, rank, core.config.simulation(), base_seed)
    }
}

#[cfg(test)]
mod tests {
    use super::{ControllerScenario, Scenario};
    use crate::simulation::config::{Config, PartitionMethod};
    use crate::simulation::scenario::network::Network;
    use crate::simulation::scenario::population::Population;
    use crate::simulation::scenario::vehicles::Garage;
    use macros::integration_test;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[integration_test]
    fn split_and_merge_mobsim_population_keeps_every_person_once() {
        let mut garage = Garage::from_file(&PathBuf::from("./assets/3-links/vehicles.xml"));
        let population = Population::from_file("./assets/3-links/3-agent.xml", &mut garage);
        let original_len = population.persons.len();
        let config = Arc::new(Config::default());
        let network = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            config.partitioning().num_parts,
            &PartitionMethod::None,
        );

        let mut scenario: ControllerScenario = Scenario {
            network,
            garage,
            population,
            config,
        }
        .into();

        let inputs = scenario.split_for_mobsim();

        assert!(scenario.population.persons.is_empty());

        let shards = inputs.into_iter().map(|input| input.population).collect();
        scenario.merge_population_shards(shards);

        assert_eq!(original_len, scenario.population.persons.len());
    }
}
