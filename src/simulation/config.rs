use std::any::Any;
use std::cell::RefCell;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use ahash::HashMap;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArgs {
    #[arg(long, short)]
    pub config_path: String,
}

#[derive(Serialize, Deserialize)]
pub struct Config2 {
    modules: RefCell<HashMap<String, Box<dyn ConfigModule>>>,
}

impl Config2 {
    pub fn from_file(path: &Path) -> Self {
        let reader = BufReader::new(File::open(path).expect("Failed to open file."));
        let config: Config2 = serde_yaml::from_reader(reader).expect("Failed to parse config.");
        config
    }

    pub fn proto_files(&self) -> ProtoFiles {
        if let Some(proto_files) = self.module::<ProtoFiles>("protofiles") {
            proto_files
        } else {
            panic!("Protofiles were not set.")
        }
    }

    pub fn set_proto_files(&mut self, proto_files: ProtoFiles) {
        self.modules
            .get_mut()
            .insert("protofiles".to_string(), Box::new(proto_files));
    }

    pub fn partitioning(&self) -> Partitioning {
        if let Some(partitioning) = self.module::<Partitioning>("partitioning") {
            partitioning
        } else {
            let default = Partitioning {
                num_parts: 1,
                method: PartitionMethod::None,
            };
            self.modules
                .borrow_mut()
                .insert("partitioning".to_string(), Box::new(default.clone()));
            default
        }
    }

    pub fn output(&self) -> Output {
        if let Some(output) = self.module::<Output>("output") {
            output
        } else {
            let default = Output {
                output_dir: "./".to_string(),
            };
            self.modules
                .borrow_mut()
                .insert("output".to_string(), Box::new(default.clone()));
            default
        }
    }

    pub fn simulation(&self) -> Simulation {
        if let Some(simulation) = self.module::<Simulation>("simulation") {
            simulation
        } else {
            let default = Simulation {
                start_time: 0,
                end_time: 86400,
                sample_size: 1.,
            };
            self.modules
                .borrow_mut()
                .insert("simulation".to_string(), Box::new(default.clone()));
            default
        }
    }

    pub fn routing(&self) -> Routing {
        if let Some(routing) = self.module::<Routing>("routing") {
            routing
        } else {
            let default = Routing {
                mode: RoutingMode::UsePlans,
            };
            self.modules
                .borrow_mut()
                .insert("routing".to_string(), Box::new(default.clone()));
            default
        }
    }

    fn module<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        if let Some(boxed) = self.modules.borrow().get(key) {
            Some(boxed.as_ref().as_any().downcast_ref::<T>().unwrap().clone())
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProtoFiles {
    pub network: String,
    pub population: String,
    pub vehicles: String,
    pub ids: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Partitioning {
    pub num_parts: u32,
    pub method: PartitionMethod,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Output {
    pub output_dir: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Routing {
    pub mode: RoutingMode,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Simulation {
    pub start_time: u32,
    pub end_time: u32,
    pub sample_size: f32,
}

#[typetag::serde(tag = "type")]
pub trait ConfigModule {
    fn as_any(&self) -> &dyn Any;
}

#[typetag::serde]
impl ConfigModule for ProtoFiles {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[typetag::serde]
impl ConfigModule for Partitioning {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[typetag::serde]
impl ConfigModule for Output {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[typetag::serde]
impl ConfigModule for Routing {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[typetag::serde]
impl ConfigModule for Simulation {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    #[arg(long, default_value_t = 0)]
    pub start_time: u32,
    #[arg(long, default_value_t = 86400)]
    pub end_time: u32,
    #[arg(long, default_value_t = 1)]
    pub num_parts: u32,
    #[arg(long)]
    pub network_file: String,
    #[arg(long)]
    pub population_file: String,
    #[arg(long)]
    pub vehicles_file: String,
    #[arg(long)]
    pub vehicle_definitions_file: Option<String>,
    #[arg(long, value_enum, default_value_t = RoutingMode::UsePlans)]
    pub routing_mode: RoutingMode,
    #[arg(long, default_value = "./")]
    pub output_dir: String,
    #[arg(long, default_value = "file")]
    pub events_mode: String,
    #[arg(long, default_value_t = 1.0)]
    pub sample_size: f32,
    #[arg(long, value_enum, default_value_t = PartitionMethod::Metis)]
    pub partition_method: PartitionMethod,
}

#[derive(PartialEq, Debug, ValueEnum, Clone, Copy, Serialize, Deserialize)]
pub enum RoutingMode {
    AdHoc,
    UsePlans,
}

#[derive(PartialEq, Debug, ValueEnum, Clone, Copy, Serialize, Deserialize)]
pub enum PartitionMethod {
    Metis,
    None,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }
}

pub struct ConfigBuilder {
    start_time: u32,
    end_time: u32,
    num_parts: u32,
    network_file: String,
    population_file: String,
    vehicles_file: String,
    vehicle_definitions_file: Option<String>,
    routing_mode: RoutingMode,
    output_dir: String,
    events_mode: String,
    sample_size: f32,
    partition_method: PartitionMethod,
}

impl ConfigBuilder {
    fn new() -> Self {
        ConfigBuilder {
            network_file: String::from(""),
            population_file: String::from(""),
            vehicles_file: String::from(""),
            vehicle_definitions_file: None,
            output_dir: String::from("./"),
            events_mode: String::from("file"),
            num_parts: 1,
            start_time: 0,
            end_time: 86400,
            sample_size: 1.0,
            routing_mode: RoutingMode::UsePlans,
            partition_method: PartitionMethod::Metis,
        }
    }

    pub fn start_time(mut self, time: u32) -> Self {
        self.start_time = time;
        self
    }

    pub fn end_time(mut self, time: u32) -> Self {
        self.end_time = time;
        self
    }

    pub fn num_parts(mut self, num_parts: u32) -> Self {
        self.num_parts = num_parts;
        self
    }

    pub fn network_file(mut self, file: String) -> Self {
        self.network_file = file;
        self
    }

    pub fn population_file(mut self, file: String) -> Self {
        self.population_file = file;
        self
    }

    pub fn vehicles_file(mut self, file: String) -> Self {
        self.vehicles_file = file;
        self
    }

    pub fn output_dir(mut self, dir: String) -> Self {
        self.output_dir = dir;
        self
    }

    pub fn events_mode(mut self, mode: String) -> Self {
        self.events_mode = mode;
        self
    }

    pub fn sample_size(mut self, sample_size: f32) -> Self {
        self.sample_size = sample_size;
        self
    }

    pub fn partition_method(mut self, method: PartitionMethod) -> Self {
        self.partition_method = method;
        self
    }

    pub fn routing_mode(mut self, routing_mode: RoutingMode) -> Self {
        self.routing_mode = routing_mode;
        self
    }

    pub fn set_vehicle_definitions_file(
        mut self,
        vehicle_definitions_file: Option<String>,
    ) -> Self {
        self.vehicle_definitions_file = vehicle_definitions_file;
        self
    }

    pub fn build(self) -> Config {
        Config {
            start_time: self.start_time,
            end_time: self.end_time,
            num_parts: self.num_parts,
            network_file: self.network_file,
            population_file: self.population_file,
            vehicles_file: self.vehicles_file,
            vehicle_definitions_file: self.vehicle_definitions_file,
            routing_mode: self.routing_mode,
            output_dir: self.output_dir,
            events_mode: self.events_mode,
            sample_size: self.sample_size,
            partition_method: self.partition_method,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::{Config2, PartitionMethod, Partitioning};

    #[test]
    fn read_from_yaml() {
        let mut config = Config2 {
            modules: Default::default(),
        };
        let partitioning = Partitioning {
            num_parts: 1,
            method: PartitionMethod::Metis,
        };
        config
            .modules
            .borrow_mut()
            .insert("partitioning".to_string(), Box::new(partitioning));

        let yaml = serde_yaml::to_string(&config).expect("Failed to serialize yaml");

        println!("{yaml}");

        let parsed_config: Config2 = serde_yaml::from_str(&yaml).expect("failed to parse config");
        println!("done.")
    }
}
