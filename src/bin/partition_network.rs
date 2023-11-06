use std::fs;
use std::path::PathBuf;

use clap::{arg, Parser};
use tracing::info;

use rust_q_sim::simulation::network::global_network::Network;

fn main() {
    rust_q_sim::simulation::logging::init_std_out_logging();
    let args = InputArgs::parse();

    fs::create_dir_all(&args.out_path).expect("Failed to create output path");

    info!(
        "Partition network: {} into {} parts",
        args.in_path, args.num_parts
    );

    let net1 = Network::from_file(&args.in_path, args.num_parts, "metis");
    info!(
        "Network is loaded with {} links and {} nodes.",
        net1.links.len(),
        net1.nodes.len()
    );

    info!("Writing to {}", args.out_path);
    net1.to_file(&PathBuf::from(&args.out_path));

    info!("Finished partitioning Network.")
}

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(short, long)]
    pub in_path: String,
    #[arg(short, long)]
    pub out_path: String,
    #[arg(long)]
    pub num_parts: u32,
}
