use crate::io::matsim_id::MatsimId;
use crate::io::network::{IONetwork, IONode};
use crate::io::population::{IOPlanElement, IOPopulation};
use crate::parallel_simulation::id_mapping::{MatsimIdMapping, MatsimIdMappings};
use log::info;
use metis::{Graph, Idx};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
struct PartitionNode {
    weight: i32,
    out_links: Vec<usize>,
    in_links: Vec<usize>,
}

#[derive(Debug)]
struct PartitionLink {
    weight: i32,
    to: usize,
    from: usize,
}

#[derive(Debug)]
pub struct PartitionInfo {
    partition_result: Vec<Idx>,
    node_id_mapping: Arc<MatsimIdMapping>,
}

impl PartitionInfo {
    pub fn from_io(
        io_network: &IONetwork,
        io_population: &IOPopulation,
        id_mappings: &MatsimIdMappings,
        num_parts: usize,
    ) -> PartitionInfo {
        if num_parts == 1 {
            info!("PartitionInfo: 'num_parts' is 1. No partitioning necessary. Put all nodes into partition 0.");
            return PartitionInfo {
                partition_result: vec![0; io_network.nodes().len()],
                node_id_mapping: id_mappings.nodes.clone(),
            };
        }
        info!("PartitionInfo: calculating node and link weights.");
        let (node_weights, link_weights) =
            PartitionInfo::calculate_weights(io_network, io_population, id_mappings);
        info!("PartitionInfo: converting nodes to partition nodes");
        let mut partition_nodes =
            PartitionInfo::create_partition_nodes(io_network, &node_weights, &id_mappings.nodes);
        info!("PartitionInfo: converting links to partition links");
        let partition_links = PartitionInfo::create_partition_links(
            io_network,
            &link_weights,
            &mut partition_nodes,
            id_mappings,
        );

        info!("PartitionInfo: starting Partitioning.");
        let partition_result =
            PartitionInfo::partition(partition_nodes, partition_links, num_parts as Idx);

        info!("PartitionInfo: finished Partitioning.");
        PartitionInfo {
            partition_result,
            node_id_mapping: id_mappings.nodes.clone(),
        }
    }

    pub fn get_partition(&self, node: &IONode) -> usize {
        let internal = *self.node_id_mapping.get_internal(node.id()).unwrap();
        *self.partition_result.get(internal).unwrap() as usize
    }

    fn partition(nodes: Vec<PartitionNode>, links: Vec<PartitionLink>, num_parts: Idx) -> Vec<Idx> {
        let mut xadj: Vec<Idx> = Vec::from([0]);
        let mut adjncy: Vec<Idx> = Vec::new();
        let mut adjwgt: Vec<Idx> = Vec::new();
        let mut vwgt: Vec<Idx> = Vec::new();
        let mut result: Vec<Idx> = vec![0x00; nodes.len()];

        info!("PartitionInfo: converting nodes and links to ajacency format for metis.");
        for node in nodes {
            let num_out_links = node.out_links.len() as Idx;
            let num_in_links = node.in_links.len() as Idx;
            let next_adjacency_index = xadj.last().unwrap() + num_out_links + num_in_links;
            xadj.push(next_adjacency_index);
            vwgt.push(node.weight as Idx);

            for id in node.out_links {
                let link = links.get(id).unwrap();
                adjncy.push(link.to as Idx);
                adjwgt.push(link.weight as Idx);
            }

            for id in node.in_links {
                let link = links.get(id).unwrap();
                adjncy.push(link.from as Idx);
                adjwgt.push(link.weight as Idx);
            }
        }

        info!("PartitionInfo: Calling Metis Partitioning Library.");
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
                    in_links: Vec::new(),
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

                // put link into out links list of from node
                let from_node_id = id_mappings.nodes.get_internal(link.from.as_str()).unwrap();
                let from_node = nodes.get_mut(*from_node_id).unwrap();
                from_node.out_links.push(*link_id);

                // put link into in links list of to node
                let to_node_id = id_mappings.nodes.get_internal(link.to.as_str()).unwrap();
                let to_node = nodes.get_mut(*to_node_id).unwrap();
                to_node.in_links.push(*link_id);

                // add weight for link
                let weight = *link_weights.get(link_id).unwrap_or(&1);

                PartitionLink {
                    to: *to_node_id,
                    from: *from_node_id,
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
