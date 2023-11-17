use std::path::{Path, PathBuf};

use clap::{arg, Parser};
use tracing::info;

use rust_q_sim::simulation::config::PartitionMethod;
use rust_q_sim::simulation::network::global_network::Network;

fn main() {
    rust_q_sim::simulation::logging::init_std_out_logging();
    let args = InputArgs::parse();

    let input_path = PathBuf::from(&args.in_path);
    let folder = input_path.parent().unwrap();
    let mut name_parts: Vec<&str> = input_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .split(".")
        .collect();
    let num_parts_string = args.num_parts.to_string();
    name_parts.insert(name_parts.len() - 2, num_parts_string.as_str());
    let out_path = folder.join(name_parts.join("."));
    info!("Writing to {:?}", out_path);
    name_parts.insert(name_parts.len() - 3, "internal-ids");
    let out_path_internal = folder.join(name_parts.join("."));
    info!("Writing to {:?}", out_path_internal);

    info!(
        "Partition network: {} into {} parts",
        args.in_path, args.num_parts
    );

    let net1 = Network::from_file(&args.in_path, args.num_parts, PartitionMethod::Metis);
    info!(
        "Network is loaded with {} links and {} nodes.",
        net1.links.len(),
        net1.nodes.len()
    );

    net1.to_file(&out_path);
    to_file_internal_ids(&net1, &out_path_internal);

    info!("Finished partitioning Network.")
}

fn to_file_internal_ids(network: &Network, file_path: &Path) {
    /*let mut result = IONetwork::new(None);

    for node in &network.nodes {
        let attributes = Attrs {
            attributes: vec![Attr {
                name: String::from("partition"),
                value: node.partition.to_string(),
                class: String::from("java.lang.Integer"),
            }],
        };
        let io_node = IONode {
            id: node.id.internal().to_string(),
            x: node.x,
            y: node.y,
            attributes: Some(attributes),
        };
        result.nodes_mut().push(io_node);
    }

    for link in &network.links {
        let modes = link
            .modes
            .iter()
            .map(|m| m.external().to_string())
            .reduce(|modes, mode| format!("{modes},{mode}"))
            .unwrap();
        let attributes = Attrs {
            attributes: vec![Attr {
                name: String::from("partition"),
                value: link.partition.to_string(),
                class: String::from("java.lang.Integer"),
            }],
        };

        let io_link = IOLink {
            id: link.id.internal().to_string(),
            from: link.from.internal().to_string(),
            to: link.to.external().to_string(),
            length: link.length,
            capacity: link.capacity,
            freespeed: link.freespeed,
            permlanes: link.permlanes,
            modes,
            attributes: Some(attributes),
        };
        result.links.effective_cell_size = Some(network.effective_cell_size);
        result.links_mut().push(io_link);
    }

    result.to_file(file_path);

     */
}

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(short, long)]
    pub in_path: String,
    #[arg(long)]
    pub num_parts: u32,
}
