use crate::simulation::config::VertexWeight::InLinkCapacity;
use crate::simulation::io::is_url;
use ahash::HashMap;
use clap::{Parser, ValueEnum};
use dyn_clone::DynClone;
#[cfg(feature = "http")]
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{info, warn, Level};

/// Macro to register an override handler for a specific config key
#[macro_export]
macro_rules! register_override {
    ($key:literal, $func:expr) => {
        inventory::submit! {
            $crate::simulation::config::OverrideHandler {
                key: $key,
                apply: $func,
            }
        }
    };
}

struct OverrideHandler {
    key: &'static str,
    apply: fn(config: &mut Config, value: &str),
}

// Collect all OverrideHandler submitted from anywhere in the crate
inventory::collect!(OverrideHandler);

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArgs {
    #[arg(long, short)]
    pub config: String,
    #[arg(long= "set", value_parser = parse_key_val)]
    pub overrides: Vec<(String, String)>,
}

impl CommandLineArgs {
    pub fn new_with_path(path: impl ToString) -> Self {
        CommandLineArgs {
            config: path.to_string(),
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    //this is deliberately a Mutex to allow for thread-safe sharing of the config
    modules: Mutex<HashMap<String, Box<dyn ConfigModule>>>,
    #[serde(skip)]
    context: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            modules: Mutex::new(HashMap::default()),
            context: None,
        }
    }
}

impl From<CommandLineArgs> for Config {
    fn from(args: CommandLineArgs) -> Self {
        let mut config = Config::from(args.config.parse::<PathBuf>().unwrap());
        config.apply_overrides(&args.overrides);
        config
    }
}

impl From<PathBuf> for Config {
    fn from(config_path: PathBuf) -> Self {
        let reader: Box<dyn BufRead>;

        // Check if the path is a URL
        let path = &config_path.to_string_lossy();
        if is_url(path) {
            #[cfg(feature = "http")]
            {
                reader = Self::url_file_reader(path.parse().unwrap());
            }
            #[cfg(not(feature = "http"))]
            {
                panic!("HTTP support is not enabled. Please recompile with the `http` feature enabled.");
            }
        } else {
            reader = Self::local_file_reader(&config_path);
        }

        // Parse YAML into Config
        let mut config: Config = serde_yaml::from_reader(reader).unwrap_or_else(|e| {
            panic!(
                "Failed to parse config at {:?}. Original error was: {}",
                config_path, e
            )
        });
        config.set_context(Some(config_path.clone()));
        config
    }
}

impl Config {
    pub fn set_context(&mut self, context: Option<PathBuf>) {
        self.context = context;
    }

    /// Apply generic key-value overrides to the config, e.g. protofiles.population=path
    fn apply_overrides(&mut self, overrides: &[(String, String)]) {
        info!("Applying overrides: {:?}", overrides);

        for (key, value) in overrides {
            let key_str = key.as_str();

            if let Some(handler) = inventory::iter::<OverrideHandler>().find(|h| h.key == key_str) {
                (handler.apply)(self, value);
            } else {
                warn!("No override handler found for key: {}", key);
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
            .lock()
            .unwrap()
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
                .lock()
                .unwrap()
                .insert("partitioning".to_string(), Box::new(default.clone()));
            default
        }
    }

    pub fn set_partitioning(&mut self, partitioning: Partitioning) {
        self.modules
            .lock()
            .unwrap()
            .insert("partitioning".to_string(), Box::new(partitioning));
    }

    pub fn set_computational_setup(&mut self, setup: ComputationalSetup) {
        self.modules
            .lock()
            .unwrap()
            .insert("computational_setup".to_string(), Box::new(setup));
    }

    pub fn set_simulation(&mut self, simulation: Simulation) {
        self.modules
            .lock()
            .unwrap()
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
                .lock()
                .unwrap()
                .insert("output".to_string(), Box::new(default.clone()));
            default
        }
    }

    pub fn set_output(&mut self, output: Output) {
        self.modules
            .lock()
            .unwrap()
            .insert("output".to_string(), Box::new(output));
    }

    pub fn set_routing(&mut self, routing: Routing) {
        self.modules
            .lock()
            .unwrap()
            .insert("routing".to_string(), Box::new(routing));
    }

    pub fn simulation(&self) -> Simulation {
        if let Some(simulation) = self.module::<Simulation>("simulation") {
            simulation
        } else {
            let default = Simulation::default();
            self.modules
                .lock()
                .unwrap()
                .insert("simulation".to_string(), Box::new(default.clone()));
            default
        }
    }

    pub fn routing(&self) -> Routing {
        if let Some(routing) = self.module::<Routing>("routing") {
            routing
        } else {
            let default = Routing::default();
            self.modules
                .lock()
                .unwrap()
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
                .lock()
                .unwrap()
                .insert("computational_setup".to_string(), Box::new(default));
            default
        }
    }

    fn module<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        self.modules
            .lock()
            .unwrap()
            .get(key)
            .map(|boxed| boxed.as_ref().as_any().downcast_ref::<T>().unwrap().clone())
    }

    pub fn context(&self) -> &Option<PathBuf> {
        &self.context
    }

    fn local_file_reader(config_path: &PathBuf) -> Box<dyn BufRead> {
        // Open the config file from the local file system
        let file = File::open(&config_path).unwrap_or_else(|e| {
            panic!(
                "Failed to open config file at {:?}. Original error was {}",
                config_path, e
            );
        });
        // Wrap the file in a BufReader for YAML parsing
        Box::new(BufReader::new(file))
    }

    #[cfg(feature = "http")]
    fn url_file_reader(url: Url) -> Box<dyn BufRead> {
        // Make a blocking request to get the config file and read the response body
        let resp = reqwest::blocking::get(url).expect("Failed to fetch config URL");
        let bytes = resp.bytes().expect("Failed to read response body").to_vec();
        // Wrap the response bytes in a BufReader for YAML parsing
        Box::new(BufReader::new(std::io::Cursor::new(bytes)))
    }
}

pub fn write_config(config: &Config, output_path: PathBuf) {
    let output_config = output_path.join("output_config.yml");
    let file = File::create(&output_config).expect("Failed to create output config file");
    let writer = BufWriter::new(file);
    serde_yaml::to_writer(writer, config).expect("Failed to write output config file");
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProtoFiles {
    pub network: PathBuf,
    pub population: PathBuf,
    pub vehicles: PathBuf,
    pub ids: PathBuf,
}

register_override!("protofiles.network", |config, value| {
    let mut proto_files = config.proto_files();
    proto_files.network = PathBuf::from(value);
    config.set_proto_files(proto_files);
});

register_override!("protofiles.population", |config, value| {
    let mut proto_files = config.proto_files();
    proto_files.population = PathBuf::from(value);
    config.set_proto_files(proto_files);
});

register_override!("protofiles.vehicles", |config, value| {
    let mut proto_files = config.proto_files();
    proto_files.vehicles = PathBuf::from(value);
    config.set_proto_files(proto_files);
});

register_override!("protofiles.ids", |config, value| {
    let mut proto_files = config.proto_files();
    proto_files.ids = PathBuf::from(value);
    config.set_proto_files(proto_files);
});

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Partitioning {
    pub num_parts: u32,
    pub method: PartitionMethod,
}

register_override!("partitioning.num_parts", |config, value| {
    let mut part = config.partitioning();
    if let Ok(v) = value.parse() {
        part.num_parts = v;
        config.set_partitioning(part);
        // replace some configuration if we get a partition from the outside. This is interesting for testing
        let out_dir = format!("{}-{v}", config.output().output_dir.to_str().unwrap());
        let mut output = config.output().clone();
        output.output_dir = out_dir.into();
        config.set_output(output);
    }
});

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

register_override!("output.output_dir", |config, value| {
    let mut output = config.output();
    output.output_dir = PathBuf::from(value);
    config.set_output(output);
});

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Routing {
    pub mode: RoutingMode,
}

register_override!("routing.mode", |config, value| {
    let mut routing = config.routing();
    routing.mode = match value.to_lowercase().as_str() {
        "ad-hoc" | "adhoc" => RoutingMode::AdHoc,
        "use-plans" | "useplans" => RoutingMode::UsePlans,
        _ => panic!("Invalid routing mode: {}", value),
    };
    config.set_routing(routing);
});

impl Default for Routing {
    fn default() -> Self {
        Routing {
            mode: RoutingMode::UsePlans,
        }
    }
}

fn default_to_3() -> u32 {
    3
}

fn default_to_600() -> u64 {
    600
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
    #[serde(default = "default_to_10")]
    pub stuck_threshold: u32,
    pub main_modes: Vec<String>,
}

fn default_to_10() -> u32 {
    10
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct ComputationalSetup {
    pub global_sync: bool,
    #[serde(default = "default_to_3")]
    /// The number of threads to be used for the tokio runtime by the adapter.
    pub adapter_worker_threads: u32,
    #[serde(default = "default_to_600")]
    pub retry_time_seconds: u64,
}

register_override!(
    "computational_setup.adapter_worker_threads",
    |config, value| {
        let mut setup = config.computational_setup();
        setup.adapter_worker_threads = value.parse().unwrap();
        config.set_computational_setup(setup);
    }
);

register_override!("computational_setup.global_sync", |config, value| {
    let mut setup = config.computational_setup();
    setup.global_sync = value.parse().unwrap();
});

impl Default for ComputationalSetup {
    fn default() -> Self {
        Self {
            global_sync: false,
            adapter_worker_threads: default_to_3(),
            retry_time_seconds: default_to_600(),
        }
    }
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
    Parquet(ParquetProfilingLevel),
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
pub struct ParquetProfilingLevel {
    #[serde(default = "default_profiling_level")]
    pub level: String,
    #[serde(default = "default_parquet_batch_size")]
    pub batch_size: usize,
}

fn default_parquet_batch_size() -> usize {
    50_000
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

impl ParquetProfilingLevel {
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
    use crate::simulation::config::Output;
    use crate::simulation::config::PathBuf;
    use crate::simulation::config::Profiling;
    use crate::simulation::config::ProtoFiles;
    use crate::simulation::config::WriteEvents;
    use crate::simulation::config::{
        parse_key_val, CommandLineArgs, ComputationalSetup, Config, Drt, DrtProcessType,
        DrtService, EdgeWeight, MetisOptions, PartitionMethod, Partitioning, Simulation,
        VertexWeight,
    };
    use crate::simulation::config::{Logging, RoutingMode};
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
        let computational_setup = ComputationalSetup {
            global_sync: true,
            adapter_worker_threads: 42,
            retry_time_seconds: 41,
        };

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
        assert_eq!(
            parsed_config.computational_setup().adapter_worker_threads,
            42
        );
        assert_eq!(parsed_config.computational_setup().retry_time_seconds, 41);

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
            .lock()
            .unwrap()
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
            config: file.path().to_str().unwrap().to_string(),
            overrides: vec![("protofiles.population".to_string(), "new_pop".to_string())],
        };
        let config = Config::from(args);
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
            config: file.path().to_str().unwrap().to_string(),
            overrides: vec![("output.output_dir".to_string(), "new_out".to_string())],
        };
        let config = Config::from(args);
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
            config: file.path().to_str().unwrap().to_string(),
            overrides: vec![("partitioning.num_parts".to_string(), "5".to_string())],
        };
        let config = Config::from(args);
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

    fn base_config() -> Config {
        let mut config = Config::default();
        config.set_proto_files(ProtoFiles {
            network: "net".into(),
            population: "pop".into(),
            vehicles: "veh".into(),
            ids: "ids".into(),
        });
        config.set_output(Output {
            output_dir: "out".into(),
            profiling: Profiling::None,
            logging: Logging::Info,
            write_events: WriteEvents::None,
        });
        config.set_partitioning(Partitioning {
            num_parts: 1,
            method: PartitionMethod::None,
        });
        config.set_routing(crate::simulation::config::Routing {
            mode: RoutingMode::UsePlans,
        });
        config
    }

    #[test]
    fn override_protofiles_network() {
        let mut config = base_config();
        config.apply_overrides(&[("protofiles.network".to_string(), "new_net".to_string())]);
        assert_eq!(config.proto_files().network, PathBuf::from("new_net"));
    }

    #[test]
    fn override_partitioning_num_parts() {
        let mut config = base_config();
        config.apply_overrides(&[("partitioning.num_parts".to_string(), "7".to_string())]);
        assert_eq!(config.partitioning().num_parts, 7);
    }

    #[test]
    fn override_routing_mode() {
        let mut config = base_config();
        config.apply_overrides(&[("routing.mode".to_string(), "ad-hoc".to_string())]);
        assert_eq!(config.routing().mode, RoutingMode::AdHoc);
    }

    #[test]
    #[should_panic]
    fn override_routing_mode_invalid() {
        let mut config = base_config();
        config.apply_overrides(&[("routing.mode".to_string(), "InvalidMode".to_string())]);
    }
}
