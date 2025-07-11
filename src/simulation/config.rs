use ahash::HashMap;
use clap::{Parser, ValueEnum};
use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::cell::RefCell;
use std::fmt::Debug;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use tracing::Level;

use crate::simulation::config::VertexWeight::InLinkCapacity;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArgs {
    #[arg(long, short)]
    pub config_path: String,
    #[arg(long= "set", value_parser = parse_key_val, number_of_values = 1)]
    pub overrides: Vec<(String, String)>,
}

impl CommandLineArgs {
    pub fn new_with_path(path: impl ToString) -> Self {
        CommandLineArgs {
            config_path: path.to_string(),
            overrides: Vec::new(),
        }
    }
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s.find('=');
    match pos {
        Some(pos) => Ok((s[..pos].to_string(), s[pos + 1..].to_string())),
        None => Err(format!("invalid KEY=VALUE: no `=` found in `{}`", s)),
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    modules: RefCell<HashMap<String, Box<dyn ConfigModule>>>,
    #[serde(skip)]
    context: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            modules: RefCell::new(HashMap::default()),
            context: None,
        }
    }
}

impl Config {
    pub fn from_file(args: &CommandLineArgs) -> Self {
        let reader = BufReader::new(File::open(&args.config_path).unwrap_or_else(|e| {
            panic!(
                "Failed to open config file at {}. Original error was {}",
                args.config_path, e
            );
        }));
        let mut config: Config = serde_yaml::from_reader(reader).unwrap_or_else(|e| {
            panic!(
                "Failed to parse config at {}. Original error was: {}",
                args.config_path, e
            )
        });
        config.set_context(Some(args.config_path.clone().parse().unwrap()));
        config.apply_overrides(&args.overrides);
        config
    }

    pub fn set_context(&mut self, context: Option<PathBuf>) {
        self.context = context;
    }

    /// Apply generic key-value overrides to the config, e.g. protofiles.population=path
    fn apply_overrides(&mut self, overrides: &[(String, String)]) {
        for (key, value) in overrides {
            let mut parts = key.splitn(2, '.');
            let module = parts.next();
            let field = parts.next();
            if let (Some(module), Some(field)) = (module, field) {
                match module {
                    "protofiles" => {
                        let mut pf = self.proto_files();
                        match field {
                            "network" => pf.network = value.into(),
                            "population" => pf.population = value.into(),
                            "vehicles" => pf.vehicles = value.into(),
                            "ids" => pf.ids = value.into(),
                            _ => continue,
                        }
                        self.set_proto_files(pf);
                    }
                    "output" => {
                        let mut out = self.output();
                        match field {
                            "output_dir" => out.output_dir = value.into(),
                            _ => continue,
                        }
                        self.set_output(out);
                    }
                    "partitioning" => {
                        let mut part = self.partitioning();
                        match field {
                            "num_parts" => {
                                if let Ok(v) = value.parse() {
                                    part.num_parts = v;
                                    // replace some configuration if we get a partition from the outside. This is interesting for testing
                                    let out_dir = format!(
                                        "{}-{v}",
                                        self.output().output_dir.to_str().unwrap()
                                    );
                                    let mut output = self.output().clone();
                                    output.output_dir = out_dir.into();
                                    self.set_output(output);
                                }
                            }
                            _ => continue,
                        }
                        self.set_partitioning(part);
                    }
                    // Add more modules/fields as needed
                    _ => continue,
                }
            }
        }
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

    pub fn set_computational_setup(&mut self, setup: ComputationalSetup) {
        self.modules
            .get_mut()
            .insert("computational_setup".to_string(), Box::new(setup));
    }

    pub fn set_simulation(&mut self, simulation: Simulation) {
        self.modules
            .get_mut()
            .insert("simulation".to_string(), Box::new(simulation));
    }

    pub fn output(&self) -> Output {
        if let Some(output) = self.module::<Output>("output") {
            output
        } else {
            let default = Output {
                output_dir: "./".parse().unwrap(),
                profiling: Profiling::None,
                logging: Logging::Info,
                write_events: WriteEvents::None,
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
            let default = Simulation::default();
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

    pub fn drt(&self) -> Option<Drt> {
        self.module::<Drt>("drt")
    }

    pub fn computational_setup(&self) -> ComputationalSetup {
        if let Some(setup) = self.module::<ComputationalSetup>("computational_setup") {
            setup
        } else {
            let default = ComputationalSetup::default();
            self.modules
                .borrow_mut()
                .insert("computational_setup".to_string(), Box::new(default));
            default
        }
    }

    fn module<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        self.modules
            .borrow()
            .get(key)
            .map(|boxed| boxed.as_ref().as_any().downcast_ref::<T>().unwrap().clone())
    }

    pub fn context(&self) -> &Option<PathBuf> {
        &self.context
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProtoFiles {
    pub network: PathBuf,
    pub population: PathBuf,
    pub vehicles: PathBuf,
    pub ids: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Partitioning {
    pub num_parts: u32,
    pub method: PartitionMethod,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Output {
    pub output_dir: PathBuf,
    #[serde(default)]
    pub profiling: Profiling,
    #[serde(default)]
    pub logging: Logging,
    #[serde(default)]
    pub write_events: WriteEvents,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Routing {
    pub mode: RoutingMode,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Drt {
    #[serde(default)]
    pub process_type: DrtProcessType,
    pub services: Vec<DrtService>,
}

#[derive(PartialEq, Debug, ValueEnum, Clone, Copy, Serialize, Deserialize, Default)]
pub enum DrtProcessType {
    #[default]
    OneProcess,
    OneProcessPerService,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DrtService {
    pub mode: String,
    #[serde(default)]
    pub stop_duration: u32,
    #[serde(default)]
    pub max_wait_time: u32,
    #[serde(default)]
    pub max_travel_time_alpha: f32,
    #[serde(default)]
    pub max_travel_time_beta: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Simulation {
    pub start_time: u32,
    pub end_time: u32,
    pub sample_size: f32,
    pub stuck_threshold: u32,
    pub main_modes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug)]
pub struct ComputationalSetup {
    pub global_sync: bool,
}

#[typetag::serde(tag = "type")]
pub trait ConfigModule: Debug + Send + DynClone {
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

#[typetag::serde]
impl ConfigModule for ComputationalSetup {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[typetag::serde]
impl ConfigModule for Drt {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// This is needed to allow cloning of the trait object and thus cloning of the Config.
dyn_clone::clone_trait_object!(ConfigModule);

impl Default for Simulation {
    fn default() -> Self {
        Self {
            start_time: 0,
            end_time: 86400,
            sample_size: 1.0,
            stuck_threshold: u32::MAX,
            main_modes: vec!["car".to_string()],
        }
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

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub enum Profiling {
    #[default]
    None,
    CSV(ProfilingLevel),
}

/// Have this extra layer of log level enum, as tracing subscriber has no
/// off/none option by default. At least it can't be parsed
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub enum Logging {
    #[default]
    None,
    Info,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub enum WriteEvents {
    #[default]
    None,
    Proto,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfilingLevel {
    #[serde(default = "default_profiling_level")]
    pub level: String,
}

impl ProfilingLevel {
    pub fn create_tracing_level(&self) -> Level {
        match self.level.as_str() {
            "INFO" => Level::INFO,
            "TRACE" => Level::TRACE,
            _ => panic!("{} not yet implemented as profiling level!", self.level),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct MetisOptions {
    #[serde(default = "default_vertex_weight")]
    pub vertex_weight: Vec<VertexWeight>,
    #[serde(default = "edge_weight_constant")]
    pub edge_weight: EdgeWeight,
    #[serde(default = "f32_value_0_03")]
    pub imbalance_factor: f32,
    #[serde(default = "u32_value_100")]
    pub iteration_number: u32,
    #[serde(default = "bool_value_false")]
    pub contiguous: bool,
}

#[derive(PartialEq, Debug, ValueEnum, Clone, Copy, Serialize, Deserialize)]
pub enum VertexWeight {
    InLinkCapacity,
    InLinkCount,
    Constant,
    PreComputed,
}

#[derive(PartialEq, Debug, ValueEnum, Clone, Copy, Serialize, Deserialize)]
pub enum EdgeWeight {
    Capacity,
    Constant,
}

impl Default for MetisOptions {
    fn default() -> Self {
        MetisOptions {
            vertex_weight: vec![],
            edge_weight: EdgeWeight::Constant,
            imbalance_factor: 0.03,
            iteration_number: 10,
            contiguous: true,
        }
    }
}

impl MetisOptions {
    pub fn set_imbalance_factor(mut self, imbalance_factor: f32) -> Self {
        self.imbalance_factor = imbalance_factor;
        self
    }

    pub fn add_vertex_weight(mut self, vertex_weight: VertexWeight) -> Self {
        self.vertex_weight.push(vertex_weight);
        self
    }

    pub fn set_edge_weight(mut self, edge_weight: EdgeWeight) -> Self {
        self.edge_weight = edge_weight;
        self
    }

    pub fn set_iteration_number(mut self, iteration_number: u32) -> Self {
        self.iteration_number = iteration_number;
        self
    }

    pub fn ufactor(&self) -> usize {
        let val = (self.imbalance_factor * 1000.) as usize;
        if val == 0 {
            return 1;
        };
        val
    }

    pub fn set_contiguous(mut self, contiguous: bool) -> Self {
        self.contiguous = contiguous;
        self
    }
}

fn f32_value_0_03() -> f32 {
    0.03
}

fn edge_weight_constant() -> EdgeWeight {
    EdgeWeight::Constant
}

fn u32_value_100() -> u32 {
    100
}

fn bool_value_false() -> bool {
    false
}

fn default_vertex_weight() -> Vec<VertexWeight> {
    vec![InLinkCapacity]
}

fn default_profiling_level() -> String {
    String::from("INFO")
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::{
        parse_key_val, CommandLineArgs, ComputationalSetup, Config, Drt, DrtProcessType,
        DrtService, EdgeWeight, MetisOptions, PartitionMethod, Partitioning, Simulation,
        VertexWeight,
    };
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn read_from_yaml() {
        let mut config = Config::default();
        let partitioning = Partitioning {
            num_parts: 1,
            method: PartitionMethod::Metis(MetisOptions {
                vertex_weight: vec![VertexWeight::InLinkCount, VertexWeight::InLinkCapacity],
                edge_weight: EdgeWeight::Constant,
                imbalance_factor: 1.02,
                iteration_number: 100,
                contiguous: true,
            }),
        };
        let computational_setup = ComputationalSetup { global_sync: true };

        let simulation = Simulation {
            start_time: 0,
            end_time: 42,
            sample_size: 0.1,
            stuck_threshold: 1,
            main_modes: vec!["bike".to_string()],
        };

        config.set_partitioning(partitioning);
        config.set_computational_setup(computational_setup);
        config.set_simulation(simulation);

        let yaml = serde_yaml::to_string(&config).expect("Failed to serialize yaml");

        println!("{yaml}");

        let parsed_config: Config = serde_yaml::from_str(&yaml).expect("failed to parse config");
        println!("done.");

        assert_eq!(parsed_config.partitioning().num_parts, 1);
        assert_eq!(
            parsed_config.partitioning().method,
            PartitionMethod::Metis(MetisOptions {
                vertex_weight: vec![VertexWeight::InLinkCount, VertexWeight::InLinkCapacity],
                edge_weight: EdgeWeight::Constant,
                imbalance_factor: 1.02,
                iteration_number: 100,
                contiguous: true,
            })
        );

        assert!(parsed_config.computational_setup().global_sync);

        assert_eq!(parsed_config.simulation().start_time, 0);
        assert_eq!(parsed_config.simulation().end_time, 42);
        assert_eq!(parsed_config.simulation().sample_size, 0.1);
        assert_eq!(parsed_config.simulation().stuck_threshold, 1);
        assert_eq!(parsed_config.simulation().main_modes, vec!["bike"]);
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
        let parsed_config: Config = serde_yaml::from_str(yaml).expect("failed to parse config");
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
              edge_weight: Capacity
        "#;
        let parsed_config: Config = serde_yaml::from_str(yaml).expect("failed to parse config");
        assert_eq!(parsed_config.partitioning().num_parts, 1);
        assert_eq!(
            parsed_config.partitioning().method,
            PartitionMethod::Metis(MetisOptions {
                vertex_weight: vec![VertexWeight::InLinkCount],
                edge_weight: EdgeWeight::Capacity,
                imbalance_factor: 1.1,
                iteration_number: 100,
                contiguous: false,
            })
        );
    }

    #[test]
    fn test_imbalance_factor() {
        assert_eq!(
            MetisOptions::default().set_imbalance_factor(0.03).ufactor(),
            30
        );
        assert_eq!(
            MetisOptions::default()
                .set_imbalance_factor(0.001)
                .ufactor(),
            1
        );
        assert_eq!(
            MetisOptions::default()
                .set_imbalance_factor(0.00001)
                .ufactor(),
            1
        );
        assert_eq!(
            MetisOptions::default()
                .set_imbalance_factor(0.00000)
                .ufactor(),
            1
        );
        assert_eq!(
            MetisOptions::default().set_imbalance_factor(-1.).ufactor(),
            1
        );
        assert_eq!(
            MetisOptions::default().set_imbalance_factor(1.1).ufactor(),
            1100
        );
    }

    #[test]
    fn test_drt() {
        let serde = r#"
        modules:
          drt:
            type: Drt
            process_type: OneProcess
            services:
              - mode: drt_a
                stop_duration: 60
                max_wait_time: 900
                max_travel_time_alpha: 1.3
                max_travel_time_beta: 600.
        "#;

        let config = Config::default();
        let drt = Drt {
            process_type: DrtProcessType::OneProcess,
            services: vec![DrtService {
                mode: "drt_a".to_string(),
                stop_duration: 60,
                max_wait_time: 900,
                max_travel_time_alpha: 1.3,
                max_travel_time_beta: 600.,
            }],
        };
        config
            .modules
            .borrow_mut()
            .insert("drt".to_string(), Box::new(drt));

        let parsed_config: Config = serde_yaml::from_str(serde).expect("failed to parse config");
        assert_eq!(
            parsed_config.drt().unwrap().process_type,
            DrtProcessType::OneProcess
        );
        assert_eq!(
            parsed_config.drt().unwrap().services[0].mode,
            "drt_a".to_string()
        );
        assert_eq!(parsed_config.drt().unwrap().services[0].stop_duration, 60);
        assert_eq!(parsed_config.drt().unwrap().services[0].max_wait_time, 900);
        assert_eq!(
            parsed_config.drt().unwrap().services[0].max_travel_time_alpha,
            1.3
        );
        assert_eq!(
            parsed_config.drt().unwrap().services[0].max_travel_time_beta,
            600.
        );
    }

    fn write_temp_config(yaml: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_override_protofiles_population() {
        let yaml = r#"
modules:
  protofiles:
    type: ProtoFiles
    network: net
    population: pop
    vehicles: veh
    ids: ids
  output:
    type: Output
    output_dir: out
"#;
        let file = write_temp_config(yaml);
        let args = CommandLineArgs {
            config_path: file.path().to_str().unwrap().to_string(),
            overrides: vec![("protofiles.population".to_string(), "new_pop".to_string())],
        };
        let config = Config::from_file(&args);
        assert_eq!(config.proto_files().population.to_str().unwrap(), "new_pop");
        assert_eq!(config.proto_files().network.to_str().unwrap(), "net");
    }

    #[test]
    fn test_override_output_dir() {
        let yaml = r#"
modules:
  protofiles:
    type: ProtoFiles
    network: net
    population: pop
    vehicles: veh
    ids: ids
  output:
    type: Output
    output_dir: out
"#;
        let file = write_temp_config(yaml);
        let args = CommandLineArgs {
            config_path: file.path().to_str().unwrap().to_string(),
            overrides: vec![("output.output_dir".to_string(), "new_out".to_string())],
        };
        let config = Config::from_file(&args);
        assert_eq!(config.output().output_dir.to_str().unwrap(), "new_out");
    }

    #[test]
    fn test_override_partitioning_num_parts() {
        let yaml = r#"
modules:
  partitioning:
    type: Partitioning
    num_parts: 1
    method: None
  output:
    type: Output
    output_dir: out
"#;
        let file = write_temp_config(yaml);
        let args = CommandLineArgs {
            config_path: file.path().to_str().unwrap().to_string(),
            overrides: vec![("partitioning.num_parts".to_string(), "5".to_string())],
        };
        let config = Config::from_file(&args);
        assert_eq!(config.partitioning().num_parts, 5);
    }

    #[test]
    fn test_parse_key_val_valid() {
        let input = "protofiles.population=some_path";
        let parsed = parse_key_val(input);
        assert_eq!(
            parsed,
            Ok(("protofiles.population".to_string(), "some_path".to_string()))
        );
    }

    #[test]
    fn test_parse_key_val_invalid() {
        let input = "protofiles.population_some_path";
        let parsed = parse_key_val(input);
        assert!(parsed.is_err());
    }
}
