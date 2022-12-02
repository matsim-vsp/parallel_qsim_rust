use std::env::Args;

#[derive(Debug)]
pub struct Config {
    pub start_time: u32,
    pub end_time: u32,
    pub num_parts: usize,
    pub network_file: String,
    pub population_file: String,
    pub output_dir: String,
    pub events_mode: String,
}

impl Config {
    pub fn from_args(args: Args) -> Config {
        let args: Vec<String> = args.collect();

        assert_eq!(8, args.len(), "One must provide 'start_time' 'end_time' 'num_parts' 'network_file' 'population_file' 'output_dir' 'events_mode' in this order");
        let result = Config {
            start_time: args.get(1).unwrap().parse().unwrap(),
            end_time: args.get(2).unwrap().parse().unwrap(),
            num_parts: args.get(3).unwrap().parse().unwrap(),
            network_file: args.get(4).unwrap().clone(),
            population_file: args.get(5).unwrap().clone(),
            output_dir: args.get(6).unwrap().clone(),
            events_mode: args.get(7).unwrap().clone(),
        };

        println!("Config is: {result:?}");
        result
    }

    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }
}

pub struct ConfigBuilder {
    start_time: u32,
    end_time: u32,
    num_parts: usize,
    network_file: String,
    population_file: String,
    output_dir: String,
    events_mode: String,
}

impl ConfigBuilder {
    fn new() -> Self {
        ConfigBuilder {
            network_file: String::from(""),
            population_file: String::from(""),
            output_dir: String::from("./"),
            events_mode: String::from("file"),
            num_parts: 0,
            start_time: 0,
            end_time: 86400,
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

    pub fn num_parts(mut self, num_parts: usize) -> Self {
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

    pub fn output_dir(mut self, dir: String) -> Self {
        self.output_dir = dir;
        self
    }

    pub fn events_mode(mut self, mode: String) -> Self {
        self.events_mode = mode;
        self
    }

    pub fn build(self) -> Config {
        Config {
            start_time: self.start_time,
            end_time: self.end_time,
            num_parts: self.num_parts,
            network_file: self.network_file,
            population_file: self.population_file,
            output_dir: self.output_dir,
            events_mode: self.events_mode,
        }
    }
}
