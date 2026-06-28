use crate::generated::general::Coordinate;
use crate::generated::network::{Link, Node};
use crate::simulation::scenario::network::Network;
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
    let wire_network = network_to_wire(network);
    crate::generated::write_to_file(wire_network, path);
    info!("Finished writing proto network to path: {path:?}");
}

pub(crate) fn network_to_wire(network: &Network) -> crate::generated::network::Network {
    info!("Converting Network into wire format");
    let nodes: Vec<_> = network
        .nodes()
        .iter()
        .map(|n| Node {
            id: n.id.external().to_string(),
            coordinate: Some(Coordinate {
                x: n.coord.x,
                y: n.coord.y,
                z: n.coord.z,
            }),
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

#[cfg(test)]
mod tests {
    use crate::simulation::id::Id;
    use crate::simulation::scenario::Coordinate;
    use crate::simulation::scenario::network::{Network, Node};
    use macros::integration_test;

    #[integration_test]
    fn node_coordinate_round_trip_preserves_none_z() {
        let mut network = Network::new();
        network.add_node(Node::new(
            Id::create("node-1"),
            Coordinate::new_2d(1.0, 2.0),
            3,
            4,
        ));

        let wire = super::network_to_wire(&network);
        let round_trip = Network::from(wire);

        let node = round_trip.get_node(&Id::get_from_ext("node-1"));
        assert_eq!(1.0, node.coord.x);
        assert_eq!(2.0, node.coord.y);
        assert_eq!(0., node.coord.z);
    }
}
