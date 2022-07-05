use crate::container::network::{IOLink, IONetwork, IONode};
use crate::parallel_simulation::id_mapping::IdMapping;
use crate::parallel_simulation::vehicles::Vehicle;
use crate::simulation::flow_cap::Flowcap;
use std::collections::{HashMap, VecDeque};

#[derive(Debug)]
pub struct Network {
    pub links: HashMap<usize, Link>,
    pub nodes: HashMap<usize, Node>,
}

impl Network {
    fn new() -> Self {
        Self {
            links: HashMap::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn split_from_container(
        container: &IONetwork,
        size: usize,
        splitter: fn(&IONode) -> usize,
    ) -> (Vec<Network>, IdMapping, IdMapping) {
        // create the result networks which can then be populated
        let mut result = Vec::with_capacity(size);

        let mut node_id_mapping = IdMapping::new();
        let mut link_id_mapping = IdMapping::new();

        for _i in 0..size {
            result.push(Network::new());
        }

        let mut next_id = 0;

        for node in container.nodes().into_iter() {
            let thread_id = splitter(node);
            let network = result.get_mut(thread_id).unwrap();

            network.add_node(next_id);
            node_id_mapping.insert(next_id, thread_id, node.id.clone());
            next_id = next_id + 1;
        }

        next_id = 0;
        for link in container.links() {
            let from_id = node_id_mapping.get_from_matsim_id(link.from.as_str());
            let to_id = node_id_mapping.get_from_matsim_id(link.to.as_str());

            let from_thread = node_id_mapping.get_thread(&from_id);
            let to_thread = node_id_mapping.get_thread(&to_id);

            if from_thread == to_thread {
                let network = result.get_mut(from_thread).unwrap();
                network.add_local_link(link, next_id, from_id, to_id);
            } else {
                let from_network = result.get_mut(from_thread).unwrap();
                from_network.add_split_out_link(next_id, from_id, from_thread, to_thread);

                let to_network = result.get_mut(to_thread).unwrap();
                to_network.add_split_in_link(link, next_id, to_id);
            }
            // the link is associated with the network which contains its to-node
            link_id_mapping.insert(next_id, to_thread, link.id.clone());
            next_id = next_id + 1;
        }

        (result, node_id_mapping, link_id_mapping)
    }

    fn add_node(&mut self, id: usize) {
        let node = Node::new(id);
        self.nodes.insert(id, node);
    }

    fn add_local_link(&mut self, link: &IOLink, id: usize, from: usize, to: usize) {
        let new_link = LocalLink {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(link.capacity),
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

    fn add_split_out_link(&mut self, id: usize, from: usize, from_thread: usize, to_thread: usize) {
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

    fn add_split_in_link(&mut self, link: &IOLink, id: usize, to: usize) {
        let new_link = LocalLink {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(link.capacity),
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

    pub fn move_vehicles(&self, links: &mut HashMap<usize, Link>, now: u32) -> Vec<ExitReason> {
        let mut exited_vehicles = Vec::new();

        for in_link_index in &self.in_links {
            if let Link::LocalLink(in_link) = links.get_mut(in_link_index).unwrap() {
                for mut vehicle in in_link.pop_front(now) {
                    vehicle.advance_route_index();
                    match vehicle.current_link_id() {
                        None => exited_vehicles.push(ExitReason::FinishRoute(vehicle)),
                        Some(out_id) => {
                            self.move_vehicle(links, *out_id, vehicle, &mut exited_vehicles, now);
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
        mut vehicle: Vehicle,
        exited_vehicles: &mut Vec<ExitReason>,
        now: u32,
    ) {
        match links.get_mut(&out_link_id).unwrap() {
            Link::LocalLink(local_link) => {
                let exit_time = now + (local_link.length / local_link.freespeed) as u32;
                vehicle.exit_time = exit_time;
                local_link.push_vehicle(vehicle);
            }
            Link::SplitLink(_) => exited_vehicles.push(ExitReason::ReachedBoundary(vehicle)),
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
    id: usize,
    q: VecDeque<Vehicle>,
    length: f32,
    freespeed: f32,
    flowcap: Flowcap,
}

impl LocalLink {
    pub fn push_vehicle(&mut self, vehicle: Vehicle) {
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
    use crate::container::network::{IONetwork, IONode};
    use crate::parallel_simulation::splittable_network::Network;

    #[test]
    fn from_container() {
        let io_network = IONetwork::from_file("./assets/equil-network.xml");
        let (split_networks, node_mapping, link_mapping) =
            Network::split_from_container(&io_network, 2, split);

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
}
