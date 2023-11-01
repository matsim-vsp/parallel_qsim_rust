use clap::{Parser, ValueEnum};

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
    #[arg(long, default_value = "metis")]
    pub partition_method: String,
}

#[derive(PartialEq, Debug, ValueEnum, Clone, Copy)]
pub enum RoutingMode {
    AdHoc,
    UsePlans,
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
    partition_method: String,
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
            num_parts: 0,
            start_time: 0,
            end_time: 86400,
            sample_size: 1.0,
            routing_mode: RoutingMode::UsePlans,
            partition_method: String::from("metis"),
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

    pub fn partition_method(mut self, method: String) -> Self {
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
