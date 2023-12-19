use std::any::Any;
use std::cell::RefCell;
use std::fs::File;
use std::io::BufReader;

use ahash::HashMap;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArgs {
    #[arg(long, short)]
    pub config_path: String,

    #[arg(long, short)]
    pub num_parts: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    modules: RefCell<HashMap<String, Box<dyn ConfigModule>>>,
}

impl Config {
    pub fn from_file(args: &CommandLineArgs) -> Self {
        let reader = BufReader::new(File::open(&args.config_path).expect("Failed to open file."));
        let mut config: Config = serde_yaml::from_reader(reader).expect("Failed to parse config.");
        // replace some configuration if we get a partition from the outside. This is interesting for testing
        if let Some(part_args) = args.num_parts {
            config.set_partitioning(Partitioning {
                num_parts: part_args,
                method: config.partitioning().method,
            });
            let out_dir = format!("{}-{part_args}", config.output().output_dir);
            config.set_output(Output {
                output_dir: out_dir,
            });
        }
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

    pub fn set_partitioning(&mut self, partitioning: Partitioning) {
        self.modules
            .get_mut()
            .insert("partitioning".to_string(), Box::new(partitioning));
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

    pub fn set_output(&mut self, output: Output) {
        self.modules
            .get_mut()
            .insert("output".to_string(), Box::new(output));
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
        self.modules
            .borrow()
            .get(key)
            .map(|boxed| boxed.as_ref().as_any().downcast_ref::<T>().unwrap().clone())
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

#[derive(PartialEq, Debug, ValueEnum, Clone, Copy, Serialize, Deserialize)]
pub enum RoutingMode {
    AdHoc,
    UsePlans,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum PartitionMethod {
    Metis(MetisOptions),
    None,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct MetisOptions {
    pub vertex_weight: Vec<VertexWeight>,
    #[serde(default = "true_value")]
    pub edge_weight: bool,
    #[serde(default = "f32_value_1_03")]
    pub imbalance_factor: f32,
}

#[derive(PartialEq, Debug, ValueEnum, Clone, Copy, Serialize, Deserialize)]
pub enum VertexWeight {
    InLinkCapacity,
    InLinkCount,
    Constant,
}

impl Default for MetisOptions {
    fn default() -> Self {
        MetisOptions {
            vertex_weight: vec![],
            edge_weight: true,
            imbalance_factor: 1.03,
        }
    }
}

impl MetisOptions {
    pub fn imbalance_factor(mut self, imbalance_factor: f32) -> Self {
        self.imbalance_factor = imbalance_factor;
        self
    }

    pub fn add_vertex_weight(mut self, vertex_weight: VertexWeight) -> Self {
        self.vertex_weight.push(vertex_weight);
        self
    }

    pub fn ufactor(&self) -> i32 {
        let val = (self.imbalance_factor * 1000. - 1000.) as i32;
        if val <= 0 {
            return 1;
        };
        val
    }

    pub fn set_edge_weight(mut self, edge_weight: bool) -> Self {
        self.edge_weight = edge_weight;
        self
    }
}

fn true_value() -> bool {
    true
}

fn f32_value_1_03() -> f32 {
    1.03
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::{
        Config, MetisOptions, PartitionMethod, Partitioning, VertexWeight,
    };

    #[test]
    fn read_from_yaml() {
        let config = Config {
            modules: Default::default(),
        };
        let partitioning = Partitioning {
            num_parts: 1,
            method: PartitionMethod::Metis(MetisOptions {
                vertex_weight: vec![VertexWeight::InLinkCount, VertexWeight::InLinkCapacity],
                edge_weight: true,
                imbalance_factor: 1.02,
            }),
        };
        config
            .modules
            .borrow_mut()
            .insert("partitioning".to_string(), Box::new(partitioning));

        let yaml = serde_yaml::to_string(&config).expect("Failed to serialize yaml");

        println!("{yaml}");

        let parsed_config: Config = serde_yaml::from_str(&yaml).expect("failed to parse config");
        println!("done.");

        assert_eq!(parsed_config.partitioning().num_parts, 1);
        assert_eq!(
            parsed_config.partitioning().method,
            PartitionMethod::Metis(MetisOptions {
                vertex_weight: vec![VertexWeight::InLinkCount, VertexWeight::InLinkCapacity],
                edge_weight: true,
                imbalance_factor: 1.02,
            })
        );
    }

    #[test]
    fn read_none_partitioning() {
        let yaml = r#"
        modules: 
          partitioning:
            type: Partitioning
            num_parts: 1
            method: None
        "#;
        let parsed_config: Config = serde_yaml::from_str(&yaml).expect("failed to parse config");
        assert_eq!(parsed_config.partitioning().num_parts, 1);
        assert_eq!(parsed_config.partitioning().method, PartitionMethod::None);
    }

    #[test]
    fn read_metis_partitioning() {
        let yaml = r#"
        modules:
          partitioning:
            type: Partitioning
            num_parts: 1
            method: !Metis
              vertex_weight: 
              - InLinkCount
              imbalance_factor: 1.1
              edge_weight: false
        "#;
        let parsed_config: Config = serde_yaml::from_str(&yaml).expect("failed to parse config");
        assert_eq!(parsed_config.partitioning().num_parts, 1);
        assert_eq!(
            parsed_config.partitioning().method,
            PartitionMethod::Metis(MetisOptions {
                vertex_weight: vec![VertexWeight::InLinkCount],
                edge_weight: false,
                imbalance_factor: 1.1,
            })
        );
    }
}
