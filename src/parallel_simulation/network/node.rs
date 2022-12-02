use crate::parallel_simulation::events::Events;
use crate::parallel_simulation::network::link::Link;
use crate::parallel_simulation::vehicles::Vehicle;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Node {
    pub id: usize,
    pub in_links: Vec<usize>,
    pub out_links: Vec<usize>,
}

pub enum ExitReason {
    FinishRoute(Vehicle),
    ReachedBoundary(Vehicle),
}

impl Node {
    pub(crate) fn new(id: usize) -> Node {
        Node {
            id,
            in_links: Vec::new(),
            out_links: Vec::new(),
        }
    }

    pub fn add_in_link(&mut self, id: usize) {
        self.in_links.push(id);
    }

    pub fn add_out_link(&mut self, id: usize) {
        self.out_links.push(id);
    }

    pub fn move_vehicles(
        &self,
        links: &mut HashMap<usize, Link>,
        now: u32,
        events: &mut Events,
    ) -> Vec<ExitReason> {
        let mut exited_vehicles = Vec::new();

        for in_link_index in &self.in_links {
            let vehicles: Vec<Vehicle> = match links.get_mut(in_link_index).unwrap() {
                Link::LocalLink(link) => link.pop_front(now),
                Link::SplitInLink(split_link) => split_link.local_link_mut().pop_front(now),
                Link::SplitOutLink(_) => panic!("No split out link expected as in link of a node."),
            };

            for mut vehicle in vehicles {
                events.handle_vehicle_leaves_link(now, *in_link_index, vehicle.id);
                vehicle.advance_route_index();
                match vehicle.current_link_id() {
                    None => exited_vehicles.push(ExitReason::FinishRoute(vehicle)),
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
            Link::SplitOutLink(_) => exited_vehicles.push(ExitReason::ReachedBoundary(vehicle)),
            Link::SplitInLink(_) => {
                panic!("Not expecting to move a vehicle onto a split in link.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parallel_simulation::events::Events;
    use crate::parallel_simulation::network::link::{Link, LocalLink};
    use crate::parallel_simulation::network::node::Node;
    use std::collections::HashMap;

    #[test]
    fn init() {
        let node = Node::new(1);

        assert_eq!(1, node.id);
        assert!(node.in_links.is_empty());
        assert!(node.out_links.is_empty());
    }

    #[test]
    fn vehicle_in() {
        let mut node = Node::new(1);
        let local_in_link = LocalLink::new(1, 20., 20., 20.);
        node.add_in_link(local_in_link.id);
        let in_link = Link::LocalLink(local_in_link);
        let mut links: HashMap<usize, Link> = HashMap::from([(1, in_link)]);
        let mut events = Events::new_none_writing();

        node.move_vehicles(&mut links, 1, &mut events);
    }

    #[test]
    fn vehicle_in_and_out() {}

    #[test]
    fn vehicles_in() {}

    #[test]
    fn vehicles_in_and_out() {}
}
