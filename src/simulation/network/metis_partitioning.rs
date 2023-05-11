use log::info;
use metis::{Idx, Graph};

use super::global_network::Network;

fn partition(network: &Network, num_parts: usize) -> Vec<Idx> {
    
    info!("Counting in links on nodes");
    // count in links
    let node_count = network
        .links
        .iter()
        .map(|l| &l.to)
        .fold(vec![0; network.nodes.len()], |mut result, id| {
            result[id.internal] += 1;
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
        vwgt.push(node_count[node.id.internal] as Idx);
        
        for id in &node.out_links {
            let link = &network.links[id.internal];
            adjncy.push(link.to.internal as Idx);
            adjwgt.push(link.capacity as Idx);
        }
        
        for id in &node.in_links {
            let link = &network.links[id.internal];
            adjncy.push(link.from.internal as Idx);
            adjwgt.push(link.capacity as Idx);
        }
    }
    
    info!("Calling Metis Partitioning Library");
    Graph::new(1, num_parts as Idx, &mut xadj, &mut adjncy)
    .set_vwgt(&mut vwgt)
    .set_option(metis::option::Seed(4711))
    .part_kway(&mut result)
    .unwrap();

    result
}

#[cfg(test)]
mod tests {
    

    use crate::simulation::{io::network::IONetwork, network::global_network::Network};

    use super::partition;

    #[test]
    fn test_equil() {
        let io_network = IONetwork::from_file("./assets/equil/equil-network.xml");
        let network = Network::from(io_network);
        
        let result = partition(&network, 4);
        
        println!("{result:#?}");
    }
    
}
