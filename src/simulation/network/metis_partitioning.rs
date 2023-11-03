use metis::{Graph, Idx};
use tracing::info;

use super::global_network::Network;

pub fn partition(network: &Network, num_parts: u32) -> Vec<Idx> {
    if num_parts <= 1 {
        return vec![0; network.nodes.len()];
    }

    info!("Counting in links on nodes");
    // count in links
    let node_count =
        network
            .links
            .iter()
            .map(|l| &l.to)
            .fold(vec![0; network.nodes.len()], |mut result, id| {
                result[id.internal() as usize] += 1;
                result
            });

    let mut xadj: Vec<Idx> = Vec::from([0]);
    let mut adjncy: Vec<Idx> = Vec::new();
    let mut adjwgt: Vec<Idx> = Vec::new();
    let mut vwgt: Vec<Idx> = Vec::new();
    let mut result: Vec<Idx> = vec![0x00; network.nodes.len()];

    info!("Converting network into Metis format");
    for node in &network.nodes {
        let num_out_links = node.out_links.len() as Idx;
        let num_in_links = node.in_links.len() as Idx;
        let next_adjacency_index = xadj.last().unwrap() + num_out_links + num_in_links;
        xadj.push(next_adjacency_index);
        vwgt.push(node_count[node.id.internal() as usize] as Idx);

        for id in &node.out_links {
            let link = &network.links[id.internal() as usize];
            adjncy.push(link.to.internal() as Idx);
            adjwgt.push(link.capacity as Idx);
        }

        for id in &node.in_links {
            let link = &network.links[id.internal() as usize];
            adjncy.push(link.from.internal() as Idx);
            adjwgt.push(link.capacity as Idx);
        }
    }

    info!("Calling Metis Partitioning Library");
    Graph::new(1, num_parts as Idx, &mut xadj, &mut adjncy)
        //.set_vwgt(&mut vwgt)
        .set_option(metis::option::Seed(4711))
        .part_kway(&mut result)
        .unwrap();

    result
}

#[cfg(test)]
mod tests {
    use crate::simulation::id2::Id;
    use crate::simulation::network::global_network::{Link, Network, Node};
    use crate::simulation::network::metis_partitioning::partition;

    #[test]
    fn simple_graph() {
        let mut net = Network::new();
        let from_id = Id::create("from");
        let to_id = Id::create("to");
        net.add_node(Node::new(from_id.clone(), 0., 0.));
        net.add_node(Node::new(to_id.clone(), 100., 0.));
        let link_id = Id::create("link");
        net.add_link(Link::new_with_default(
            link_id,
            net.get_node(&from_id),
            net.get_node(&to_id),
        ));

        for _n in 0..100 {
            let _partition_result = partition(&net, 2);
        }
    }
}
