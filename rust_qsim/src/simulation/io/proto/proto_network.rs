use crate::generated::network::{Link, Node};
use crate::simulation::network::Network;
use std::path::Path;
use tracing::info;

pub fn load_from_proto(path: &Path) -> Network {
    info!("Start reading proto network from path: {path:?}");
    let wire_net: crate::generated::network::Network = crate::generated::read_from_file(path);
    let res = Network::from(wire_net);
    info!("Finished reading proto network from path: {path:?}");
    res
}

pub fn write_to_proto(network: &Network, path: &Path) {
    info!("Start writing proto network to path: {path:?}");
    let wire_network = crate::generated::network::Network::from(network);
    crate::generated::write_to_file(wire_network, path);
    info!("Finished writing proto network to path: {path:?}");
}

impl crate::generated::network::Network {
    pub fn from(network: &Network) -> Self {
        info!("Converting Network into wire format");
        let nodes: Vec<_> = network
            .nodes()
            .iter()
            .map(|n| Node {
                id: n.id.external().to_string(),
                x: n.x,
                y: n.y,
                partition: n.partition,
                cmp_weight: n.cmp_weight,
            })
            .collect();
        let links: Vec<_> = network
            .links()
            .iter()
            .map(|l| Link {
                id: l.id.external().to_string(),
                from: l.from.external().to_string(),
                to: l.to.external().to_string(),
                length: l.length,
                capacity: l.capacity,
                freespeed: l.freespeed,
                permlanes: l.permlanes,
                modes: l.modes.iter().map(|id| id.external().to_string()).collect(),
                partition: l.partition,
            })
            .collect();

        crate::generated::network::Network {
            nodes,
            links,
            effective_cell_size: network.effective_cell_size(),
        }
    }
}
