use crate::container::matsim_id::MatsimId;
use crate::container::network::{IONetwork, IONode};
use crate::container::population::{IOPlanElement, IOPopulation};
use crate::parallel_simulation::id_mapping::{MatsimIdMapping, MatsimIdMappings};
use metis::Graph;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
struct PartitionNode {
    weight: i32,
    out_links: Vec<usize>,
}

#[derive(Debug)]
struct PartitionLink {
    weight: i32,
    to: usize,
}

#[derive(Debug)]
pub struct PartitionInfo {
    num_parts: usize,
    partition_result: Vec<i32>,
    node_id_mapping: Arc<MatsimIdMapping>,
}

impl PartitionInfo {
    pub fn from_io(
        io_network: &IONetwork,
        io_population: &IOPopulation,
        id_mappings: &MatsimIdMappings,
        num_parts: usize,
    ) -> PartitionInfo {
        println!("PartitionInfo: calculating node and link weights.");
        let (node_weights, link_weights) =
            PartitionInfo::calculate_weights(io_network, io_population, id_mappings);
        println!("PartitionInfo: converting nodes to partition nodes");
        let mut partition_nodes =
            PartitionInfo::create_partition_nodes(io_network, &node_weights, &id_mappings.nodes);
        println!("PartitionInfo: converting links to partition links");
        let partition_links = PartitionInfo::create_partition_links(
            io_network,
            &link_weights,
            &mut partition_nodes,
            id_mappings,
        );

        println!("PartitionInfo: starting Partitioning.");
        let partition_result =
            PartitionInfo::partition(partition_nodes, partition_links, num_parts as i32);

        println!("PartitionInfo: finished Partitioning.");
        PartitionInfo {
            num_parts,
            partition_result,
            node_id_mapping: id_mappings.nodes.clone(),
        }
    }

    pub fn get_partition(&self, node: &IONode) -> usize {
        let internal = *self.node_id_mapping.get_internal(node.id()).unwrap();
        *self.partition_result.get(internal).unwrap() as usize
    }

    fn partition(nodes: Vec<PartitionNode>, links: Vec<PartitionLink>, num_parts: i32) -> Vec<i32> {
        let mut xadj: Vec<i32> = Vec::from([0]);
        let mut adjncy: Vec<i32> = Vec::new();
        let mut adjwgt: Vec<i32> = Vec::new();
        let mut vwgt: Vec<i32> = Vec::new();
        let mut result = vec![0x00; nodes.len()];

        println!("PartitionInfo: converting nodes and links to ajacency format for metis.");
        for node in nodes {
            let num_out_links = node.out_links.len() as i32;
            let next_adjacency_index = xadj.last().unwrap() + num_out_links;
            xadj.push(next_adjacency_index as i32);
            vwgt.push(node.weight);

            for id in node.out_links {
                let link = links.get(id).unwrap();
                adjncy.push(link.to as i32);
                adjwgt.push(link.weight);
            }
        }

        println!("PartitionInfo: Calling Metis Partitioning Library.");
        Graph::new(1, num_parts, &mut xadj, &mut adjncy)
            // I would like to use make metis not part busy links, but this didn't work on the first try
            // come back later and figure out the details. The first attempt with only weighting nodes
            // seems to be okay for starters.
            //.set_adjwgt(&mut adjwgt)
            .set_vwgt(&mut vwgt)
            .part_kway(&mut result)
            .unwrap();

        result
    }

    fn create_partition_nodes(
        io_network: &IONetwork,
        node_weights: &HashMap<usize, i32>,
        node_id_mapping: &MatsimIdMapping,
    ) -> Vec<PartitionNode> {
        io_network
            .nodes()
            .iter()
            .map(|node| {
                let internal = node_id_mapping.get_internal(node.id.as_str()).unwrap();
                let weight = *node_weights.get(internal).unwrap_or(&1);
                PartitionNode {
                    weight,
                    out_links: Vec::new(),
                }
            })
            .collect()
    }

    fn create_partition_links(
        io_network: &IONetwork,
        link_weights: &HashMap<usize, i32>,
        nodes: &mut Vec<PartitionNode>,
        id_mappings: &MatsimIdMappings,
    ) -> Vec<PartitionLink> {
        io_network
            .links()
            .iter()
            .map(|link| {
                let link_id = id_mappings.links.get_internal(link.id.as_str()).unwrap();
                let to_node_id = id_mappings.nodes.get_internal(link.to.as_str()).unwrap();

                // put link into out links list of from node
                let from_node_id = id_mappings.nodes.get_internal(link.from.as_str()).unwrap();
                let from_node = nodes.get_mut(*from_node_id).unwrap();
                from_node.out_links.push(*link_id);
                let weight = *link_weights.get(link_id).unwrap_or(&1);

                PartitionLink {
                    to: *to_node_id,
                    weight: weight / 100, // TODO find an appropriate weight for links, or leave it out?
                }
            })
            .collect()
    }

    fn calculate_weights(
        io_network: &IONetwork,
        io_population: &IOPopulation,
        id_mappings: &MatsimIdMappings,
    ) -> (HashMap<usize, i32>, HashMap<usize, i32>) {
        let mut link_weights: HashMap<usize, i32> = HashMap::new();
        let mut node_weights: HashMap<usize, i32> = HashMap::new();

        io_population
            .persons
            .iter()
            .flat_map(|p| p.plans.iter())
            .filter(|p| p.selected)
            .flat_map(|p| p.elements.iter())
            .for_each(|el| match el {
                IOPlanElement::Activity(a) => {
                    let internal = id_mappings.links.get_internal(a.link.as_str()).unwrap();
                    PartitionInfo::increment(&mut link_weights, *internal);
                }
                IOPlanElement::Leg(l) => {
                    if l.route.r#type == "links" {
                        let route = l.route.route.as_ref().unwrap();
                        for id in route.split(' ') {
                            let internal = id_mappings.links.get_internal(id).unwrap();
                            PartitionInfo::increment(&mut link_weights, *internal);

                            let link = io_network.links().get(*internal).unwrap();
                            let internal_node_id =
                                id_mappings.nodes.get_internal(link.to.as_str()).unwrap();
                            PartitionInfo::increment(&mut node_weights, *internal_node_id);
                        }
                    }
                }
            });

        (node_weights, link_weights)
    }

    fn increment(map: &mut HashMap<usize, i32>, key: usize) {
        map.entry(key).and_modify(|w| *w += 1).or_insert(1);
    }
}
