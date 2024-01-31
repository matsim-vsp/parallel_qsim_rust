use std::path::PathBuf;

use clap::Parser;

use rust_q_sim::simulation::config::PartitionMethod;
use rust_q_sim::simulation::network::global_network::Network;
use rust_q_sim::simulation::population::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(short, long)]
    pub network: PathBuf,
    #[arg(short, long)]
    pub population: PathBuf,
    #[arg(short, long)]
    pub vehicles: PathBuf,
    #[arg(short, long)]
    pub output_dir: PathBuf,
    #[arg(short, long)]
    pub run_id: String,
}

fn main() {
    rust_q_sim::simulation::logging::init_std_out_logging();
    let args = InputArgs::parse();

    let net = Network::from_file_path(&args.network, 1, PartitionMethod::None);
    let mut veh = Garage::from_file(&args.vehicles);
    let pop = Population::from_file(&args.population, &mut veh);

    rust_q_sim::simulation::id::store_to_file(&create_file_path(&args, "ids"));
    net.to_file(&create_file_path(&args, "network"));
    veh.to_file(&create_file_path(&args, "vehicles"));
    pop.to_file(&create_file_path(&args, "plans"));
}

fn create_file_path(args: &InputArgs, extension: &str) -> PathBuf {
    args.output_dir
        .join(format!("{}.{}.binpb", args.run_id, extension))
}
