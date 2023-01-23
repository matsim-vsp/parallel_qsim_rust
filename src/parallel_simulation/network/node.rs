use crate::mpi::events::proto::Event;
use crate::mpi::events::EventsPublisher;
use crate::parallel_simulation::network::link::Link;
use std::collections::HashMap;
use std::fmt::Debug;

pub trait NodeVehicle: Debug {
    fn id(&self) -> usize;
    fn advance_route_index(&mut self);
    fn curr_link_id(&self) -> Option<usize>;
}

#[derive(Debug)]
pub struct Node {
    pub id: usize,
    pub in_links: Vec<usize>,
    pub out_links: Vec<usize>,
}

pub enum ExitReason<V> {
    FinishRoute(V),
    ReachedBoundary(V),
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

    pub fn move_vehicles<V: NodeVehicle>(
        &self,
        links: &mut HashMap<usize, Link<V>>,
        now: u32,
        events: &mut EventsPublisher,
    ) -> Vec<ExitReason<V>> {
        let mut exited_vehicles = Vec::new();

        for in_link_index in &self.in_links {
            let vehicles: Vec<V> = match links.get_mut(in_link_index).unwrap() {
                Link::LocalLink(link) => link.pop_front(now),
                Link::SplitInLink(split_link) => split_link.local_link_mut().pop_front(now),
                Link::SplitOutLink(_) => panic!("No split out link expected as in link of a node."),
            };

            for mut vehicle in vehicles {
                events.publish_event(
                    now,
                    &Event::new_link_leave(*in_link_index as u64, vehicle.id() as u64),
                );
                vehicle.advance_route_index();
                match vehicle.curr_link_id() {
                    None => exited_vehicles.push(ExitReason::FinishRoute(vehicle)),
                    Some(out_id) => {
                        self.move_vehicle(
                            links,
                            out_id,
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

    fn move_vehicle<V: NodeVehicle>(
        &self,
        links: &mut HashMap<usize, Link<V>>,
        out_link_id: usize,
        vehicle: V,
        exited_vehicles: &mut Vec<ExitReason<V>>,
        now: u32,
        events: &mut EventsPublisher,
    ) {
        match links.get_mut(&out_link_id).unwrap() {
            Link::LocalLink(local_link) => {
                events.publish_event(
                    now,
                    &Event::new_link_enter(local_link.id() as u64, vehicle.id() as u64),
                );
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

    use crate::mpi::events::EventsPublisher;
    use crate::parallel_simulation::network::link::{Link, LocalLink, SplitOutLink};
    use crate::parallel_simulation::network::node::{ExitReason, Node};
    use crate::parallel_simulation::vehicles::Vehicle;
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
        let mut local_in_link = LocalLink::new(1, 20., 40., 20., 1.);
        let vehicle = Vehicle::new(1, 1, vec![1]);
        local_in_link.push_vehicle(vehicle, 1);
        node.add_in_link(local_in_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let mut links: HashMap<usize, Link<Vehicle>> = HashMap::from([(1, in_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events);

        assert_eq!(1, exited_vehicles.len());
        let entry = exited_vehicles.get(0).unwrap();
        assert!(matches!(entry, ExitReason::FinishRoute(_)));
    }

    #[test]
    fn vehicle_in_and_out() {
        let mut node = Node::new(1);
        let mut local_in_link = LocalLink::new(1, 20., 40., 20., 1.);
        let local_out_link = LocalLink::new(2, 20., 40., 20., 1.);
        let vehicle = Vehicle::new(1, 1, vec![1, 2]);
        local_in_link.push_vehicle(vehicle, 1);
        node.add_in_link(local_in_link.id());
        node.add_out_link(local_out_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let out_link = Link::LocalLink(local_out_link);
        let mut links: HashMap<usize, Link<Vehicle>> = HashMap::from([(1, in_link), (2, out_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events);

        assert_eq!(0, exited_vehicles.len());
        let out_link_ref = links.get_mut(&2).unwrap();
        if let Link::LocalLink(local_out) = out_link_ref {
            let vehicles = local_out.pop_front(1);
            assert_eq!(1, vehicles.len());
        }
    }

    #[test]
    pub fn vehicle_in_out_boundary() {
        let mut node = Node::new(1);
        let mut local_in_link = LocalLink::new(1, 20., 40., 20., 1.);
        let split_out_link = SplitOutLink::new(2, 2);
        let vehicle = Vehicle::new(1, 1, vec![1, 2]);
        local_in_link.push_vehicle(vehicle, 1);
        node.add_in_link(local_in_link.id());
        node.add_out_link(split_out_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let out_link = Link::SplitOutLink(split_out_link);
        let mut links: HashMap<usize, Link<Vehicle>> = HashMap::from([(1, in_link), (2, out_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events);

        assert_eq!(1, exited_vehicles.len());
        assert!(matches!(
            exited_vehicles.get(0).unwrap(),
            ExitReason::ReachedBoundary(_)
        ));
    }

    #[test]
    fn vehicles_in() {
        let mut node = Node::new(1);
        let mut local_in_link = LocalLink::new(1, 3600., 40., 20., 1.);
        let vehicle_1 = Vehicle::new(1, 1, vec![1]);
        let vehicle_2 = Vehicle::new(2, 2, vec![1]);
        local_in_link.push_vehicle(vehicle_1, 1);
        local_in_link.push_vehicle(vehicle_2, 1);
        node.add_in_link(local_in_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let mut links: HashMap<usize, Link<Vehicle>> = HashMap::from([(1, in_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events);

        assert_eq!(1, exited_vehicles.len());
        let entry = exited_vehicles.get(0).unwrap();
        assert!(matches!(entry, ExitReason::FinishRoute(_)));
        if let ExitReason::FinishRoute(vehicle) = entry {
            assert_eq!(1, vehicle.id);
        }

        let exited_vehicles = node.move_vehicles(&mut links, 2, &mut events);

        assert_eq!(1, exited_vehicles.len());
        let entry = exited_vehicles.get(0).unwrap();
        assert!(matches!(entry, ExitReason::FinishRoute(_)));
        if let ExitReason::FinishRoute(vehicle) = entry {
            assert_eq!(2, vehicle.id);
        }
    }

    #[test]
    fn vehicles_in_and_out() {
        let mut node = Node::new(1);
        let mut local_in_link = LocalLink::new(1, 10000., 40., 20., 1.);
        let local_out_link = LocalLink::new(2, 10000., 40., 20., 1.);
        let vehicle_1 = Vehicle::new(1, 1, vec![1, 2]);
        let vehicle_2 = Vehicle::new(2, 2, vec![1, 2]);
        local_in_link.push_vehicle(vehicle_1, 1);
        local_in_link.push_vehicle(vehicle_2, 1);
        node.add_in_link(local_in_link.id());
        node.add_out_link(local_out_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let out_link = Link::LocalLink(local_out_link);
        let mut links: HashMap<usize, Link<Vehicle>> = HashMap::from([(1, in_link), (2, out_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events);
        assert_eq!(0, exited_vehicles.len());

        let out_link_ref = links.get_mut(&2).unwrap();
        if let Link::LocalLink(local_out) = out_link_ref {
            let vehicles = local_out.pop_front(1);
            assert_eq!(2, vehicles.len());
        }
    }
}
