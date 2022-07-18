use crate::io::matsim_id::MatsimId;
use crate::io::network::{IOLink, IONetwork, IONode};
use crate::parallel_simulation::events::Events;
use crate::parallel_simulation::id_mapping::MatsimIdMappings;
use crate::parallel_simulation::vehicles::Vehicle;
use crate::simulation::flow_cap::Flowcap;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Debug)]
pub struct Network {
    pub partitions: Vec<NetworkPartition>,
    pub nodes_2_thread: Arc<HashMap<usize, usize>>,
    pub links_2_thread: Arc<HashMap<usize, usize>>,
}

pub struct MutNetwork {
    pub partitions: Vec<NetworkPartition>,
    pub nodes_2_thread: HashMap<usize, usize>,
    pub links_2_thread: HashMap<usize, usize>,
}

impl Network {
    fn from_mut_network(network: MutNetwork) -> Network {
        Network {
            partitions: network.partitions,
            nodes_2_thread: Arc::new(network.nodes_2_thread),
            links_2_thread: Arc::new(network.links_2_thread),
        }
    }

    pub fn from_io<F>(
        io_network: &IONetwork,
        num_part: usize,
        split: F,
        id_mappings: &MatsimIdMappings,
    ) -> Network
    where
        F: Fn(&IONode) -> usize,
    {
        let mut result = MutNetwork::new(num_part);

        for node in io_network.nodes() {
            result.add_node(node, id_mappings, &split);
        }

        for link in io_network.links() {
            result.add_link(link, id_mappings);
        }

        Network::from_mut_network(result)
    }

    pub fn get_thread_for_node(&self, node_id: &usize) -> &usize {
        self.nodes_2_thread.get(node_id).unwrap()
    }

    pub fn get_thread_for_link(&self, link_id: &usize) -> &usize {
        self.links_2_thread.get(link_id).unwrap()
    }
}

impl MutNetwork {
    fn new(num_parts: usize) -> MutNetwork {
        let mut partitions = Vec::with_capacity(num_parts);
        for _ in 0..num_parts {
            partitions.push(NetworkPartition::new());
        }

        MutNetwork {
            partitions,
            nodes_2_thread: HashMap::new(),
            links_2_thread: HashMap::new(),
        }
    }

    fn add_node<F>(&mut self, node: &IONode, id_mappings: &MatsimIdMappings, split: F)
    where
        F: Fn(&IONode) -> usize,
    {
        let thread = split(node);
        let node_id = *id_mappings.nodes.get_internal(node.id()).unwrap();
        let network = self.partitions.get_mut(thread).unwrap();
        network.add_node(node_id);

        self.nodes_2_thread.insert(node_id, thread);
    }

    fn add_link(&mut self, io_link: &IOLink, id_mappings: &MatsimIdMappings) {
        let link_id = *id_mappings.links.get_internal(io_link.id()).unwrap();
        let from_id = *id_mappings
            .nodes
            .get_internal(io_link.from.as_str())
            .unwrap();
        let to_id = *id_mappings.nodes.get_internal(io_link.to.as_str()).unwrap();
        let from_thread = *self.get_thread_for_node(&from_id);
        let to_thread = *self.get_thread_for_node(&to_id);
        let to_network = self.partitions.get_mut(to_thread).unwrap();

        if from_thread == to_thread {
            to_network.add_local_link(io_link, link_id, from_id, to_id);
        } else {
            to_network.add_split_in_link(io_link, link_id, to_id);

            let from_network = self.partitions.get_mut(from_thread).unwrap();
            from_network.add_split_out_link(link_id, from_id, from_thread, to_thread);
        }
        // the link is associated with the network which contains its to-node
        self.links_2_thread.insert(link_id, to_thread);
    }

    fn get_thread_for_node(&self, node_id: &usize) -> &usize {
        self.nodes_2_thread.get(node_id).unwrap()
    }

    fn get_thread_for_link(&self, link_id: &usize) -> &usize {
        self.links_2_thread.get(link_id).unwrap()
    }
}

#[derive(Debug)]
pub struct NetworkPartition {
    pub links: HashMap<usize, Link>,
    pub nodes: HashMap<usize, Node>,
}

impl NetworkPartition {
    fn new() -> Self {
        Self {
            links: HashMap::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: usize) {
        let node = Node::new(id);
        self.nodes.insert(id, node);
    }

    pub fn add_local_link(&mut self, link: &IOLink, id: usize, from: usize, to: usize) {
        let new_link = LocalLink {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(link.capacity / 3600.),
            freespeed: link.freespeed,
            length: link.length,
        };
        self.links.insert(id, Link::LocalLink(new_link));

        // wire up the from and to node
        let from = self.nodes.get_mut(&from).unwrap();
        from.out_links.push(id);
        let to = self.nodes.get_mut(&to).unwrap();
        to.in_links.push(id);
    }

    pub fn add_split_out_link(
        &mut self,
        id: usize,
        from: usize,
        from_thread: usize,
        to_thread: usize,
    ) {
        let new_link = SplitLink {
            id,
            from_thread_id: from_thread,
            to_thread_id: to_thread,
        };
        self.links.insert(id, Link::SplitLink(new_link));

        // wire up from node
        let from_node = self.nodes.get_mut(&from).unwrap();
        from_node.out_links.push(id);
    }

    pub fn add_split_in_link(&mut self, link: &IOLink, id: usize, to: usize) {
        let new_link = LocalLink {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(link.capacity / 3600.),
            freespeed: link.freespeed,
            length: link.length,
        };
        self.links.insert(id, Link::LocalLink(new_link));

        // wire up to node
        let to_node = self.nodes.get_mut(&to).unwrap();
        to_node.in_links.push(id);
    }
}

#[derive(Debug)]
pub struct Node {
    id: usize,
    in_links: Vec<usize>,
    out_links: Vec<usize>,
}

impl Node {
    fn new(id: usize) -> Node {
        Node {
            id,
            in_links: Vec::new(),
            out_links: Vec::new(),
        }
    }

    pub fn move_vehicles(
        &self,
        links: &mut HashMap<usize, Link>,
        now: u32,
        events: &mut Events,
    ) -> Vec<ExitReason> {
        let mut exited_vehicles = Vec::new();

        for in_link_index in &self.in_links {
            if let Link::LocalLink(in_link) = links.get_mut(in_link_index).unwrap() {
                for mut vehicle in in_link.pop_front(now) {
                    //let in_link_id = in_link.id;
                    events.handle_vehicle_leaves_link(now, *in_link_index, vehicle.id);
                    vehicle.advance_route_index();
                    match vehicle.current_link_id() {
                        None => {
                            println!(
                                "Node: Vehicle #{} at the end of its route at time {now}",
                                vehicle.id
                            );
                            exited_vehicles.push(ExitReason::FinishRoute(vehicle))
                        }
                        Some(out_id) => {
                            self.move_vehicle(
                                links,
                                *out_id,
                                vehicle,
                                &mut exited_vehicles,
                                now,
                                events,
                            );
                        }
                    }
                }
            } else {
                panic!("Only expecting local links as in links")
            }
        }

        exited_vehicles
    }

    fn move_vehicle(
        &self,
        links: &mut HashMap<usize, Link>,
        out_link_id: usize,
        vehicle: Vehicle,
        exited_vehicles: &mut Vec<ExitReason>,
        now: u32,
        events: &mut Events,
    ) {
        match links.get_mut(&out_link_id).unwrap() {
            Link::LocalLink(local_link) => {
                events.handle_vehicle_enters_link(now, local_link.id, vehicle.id);
                local_link.push_vehicle(vehicle, now);
            }
            Link::SplitLink(split_link) => {
                println!(
                    "Node: Vehicle #{} at split link #{} at time {now}",
                    vehicle.id, split_link.id
                );
                exited_vehicles.push(ExitReason::ReachedBoundary(vehicle))
            }
        }
    }
}

#[derive(Debug)]
pub enum Link {
    LocalLink(LocalLink),
    SplitLink(SplitLink),
}

#[derive(Debug)]
pub struct LocalLink {
    pub id: usize,
    q: VecDeque<Vehicle>,
    length: f32,
    freespeed: f32,
    flowcap: Flowcap,
}

impl LocalLink {
    pub fn push_vehicle(&mut self, mut vehicle: Vehicle, now: u32) {
        println!(
            "LocalLink: Vehicle #{} enters link #{} at time {}",
            vehicle.id, self.id, now
        );
        let exit_time = now + (self.length / self.freespeed) as u32;
        vehicle.exit_time = exit_time;
        self.q.push_back(vehicle);
    }

    pub fn pop_front(&mut self, now: u32) -> Vec<Vehicle> {
        self.flowcap.update_capacity(now);
        let mut popped_veh = Vec::new();

        while let Some(vehicle) = self.q.front() {
            if vehicle.exit_time > now || !self.flowcap.has_capacity() {
                break;
            }

            let vehicle = self.q.pop_front().unwrap();
            self.flowcap.consume_capacity(1.0);
            println!(
                "LocalLink: Vehicle #{} leaves link #{} at time {}",
                vehicle.id, self.id, now
            );
            popped_veh.push(vehicle);
        }

        popped_veh
    }
}

#[derive(Debug)]
pub struct SplitLink {
    id: usize,
    from_thread_id: usize,
    to_thread_id: usize,
}

pub enum ExitReason {
    FinishRoute(Vehicle),
    ReachedBoundary(Vehicle),
}

#[cfg(test)]
mod tests {

    /*
    #[test]
    fn from_container() {
        let io_network = IONetwork::from_file("./assets/equil-network.xml");
        let (split_networks, node_mapping, link_mapping) =
            NetworkPartition::(&io_network, 2, split);

        assert_eq!(split_networks.len(), 2);

        let first = split_networks.get(0).unwrap();
        assert_eq!(first.nodes.len(), 8);
        assert_eq!(first.links.len(), 17);

        let second = split_networks.get(1).unwrap();
        assert_eq!(second.nodes.len(), 7);
        assert_eq!(second.links.len(), 16);

        assert_eq!(link_mapping.id_2_thread.len(), 23);
        assert_eq!(node_mapping.id_2_thread.len(), 15);
    }

    fn split(node: &IONode) -> usize {
        let node_group_1 = vec!["1", "2", "3", "4", "5", "6", "7", "15"];
        if node_group_1.contains(&node.id.as_str()) {
            0
        } else {
            1
        }
    }

     */
}
