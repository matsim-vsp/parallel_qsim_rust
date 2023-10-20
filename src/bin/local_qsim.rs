use clap::Parser;

use rust_q_sim::simulation::config::Config;
use rust_q_sim::simulation::controller;

fn main() {
    let config = Config::parse();
    if config.num_parts < 2 {
        controller::run_single_partition();
    } else {
        controller::run_channel();
    }
}
