use crate::parallel_simulation::events::EventsMode;
use std::env::Args;

#[derive(Debug)]
pub struct Config {
    pub start_time: u32,
    pub end_time: u32,
    pub num_parts: usize,
    pub network_file: String,
    pub population_file: String,
    pub output_dir: String,
    pub events_mode: EventsMode,
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
            events_mode: EventsMode::from_str(args.get(7).unwrap()),
        };

        println!("Config is: {result:?}");
        result
    }
}
