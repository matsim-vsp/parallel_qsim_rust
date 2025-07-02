use crate::simulation::network::Network;
use std::path::Path;

pub fn load_from_proto(path: &Path) -> Network {
    let wire_net: crate::simulation::io::proto::network::Network =
        crate::simulation::io::proto::read_from_file(path);
    Network::from(wire_net)
}

pub fn write_to_proto(network: &Network, path: &Path) {
    let wire_network = crate::simulation::io::proto::network::Network::from(network);
    crate::simulation::io::proto::write_to_file(wire_network, path);
}
