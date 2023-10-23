use std::path::PathBuf;

use clap::{arg, Parser};

use rust_q_sim::simulation::network::global_network::Network;

fn main() {
    let args = InputArgs::parse();
    println!(
        "Partition network: {} into {} parts",
        args.in_path, args.num_parts
    );

    let net = Network::from_file(&args.in_path, args.num_parts, "metis");
    println!(
        "Network is loaded with {} links and {} nodes.",
        net.links.len(),
        net.nodes.len()
    );
    println!("Writing to {}", args.out_path);
    net.to_file(&PathBuf::from(&args.out_path));

    println!("Finished partitioning Network.")
}

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(short, long)]
    pub in_path: String,
    #[arg(short, long)]
    pub out_path: String,
    #[arg(long, default_value_t = 1)]
    pub num_parts: u32,
}
