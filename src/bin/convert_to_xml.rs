use std::path::PathBuf;

use clap::Parser;
use tracing::info;

use rust_q_sim::simulation::config::PartitionMethod;
use rust_q_sim::simulation::network::global_network::Network;
use rust_q_sim::simulation::population::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(short, long)]
    pub network: String,
    #[arg(short, long)]
    pub population: String,
    #[arg(short, long)]
    pub vehicles: String,
    #[arg(short, long)]
    pub ids: String,
}

fn main() {
    rust_q_sim::simulation::logging::init_std_out_logging();
    let args = InputArgs::parse();
    let net_path = PathBuf::from(&args.network);
    let pop_path = PathBuf::from(&args.population);
    let veh_path = PathBuf::from(&args.vehicles);
    let ids_path = PathBuf::from(&args.ids);

    rust_q_sim::simulation::id::load_from_file(&ids_path);
    let net = Network::from_file_path(&net_path, 1, PartitionMethod::None);
    let mut veh = Garage::from_file(&veh_path);
    let pop = Population::from_file(&pop_path, &mut veh);

    net.to_file(&replace_filename(net_path));
    veh.to_file(&replace_filename(veh_path));
    pop.to_file(&replace_filename(pop_path))
}

fn replace_filename(path: PathBuf) -> PathBuf {
    let file_name = path.file_name().unwrap().to_str().unwrap();
    let stripped = file_name.strip_suffix(".binpb");
    let new_file_name = format!("{}.xml.gz", stripped.unwrap());
    info!("New file name: {}", new_file_name);
    let result = path.parent().unwrap().join(new_file_name);
    result
}
