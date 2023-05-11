use std::collections::HashMap;

use crate::simulation::{
    id::Id,
    io::vehicle_definitions::VehicleDefinitions,
    messaging::{
        events::{proto::Event, EventsPublisher},
        messages::proto::Vehicle,
    },
};

use super::{
    global_network::{Network, Node},
    link::SimLink,
    node::ExitReason,
};

pub struct SimNetwork<'n> {
    nodes: Vec<Id<Node>>,
    links: HashMap<usize, SimLink>,
    global_network: &'n Network<'n>,
}

impl<'n> SimNetwork<'n> {
    pub fn move_nodes(
        &mut self,
        events: &mut EventsPublisher,
        veh_def: Option<&VehicleDefinitions>,
        now: u32,
    ) -> Vec<ExitReason> {
        let mut exited_vehicles = Vec::new();

        for node_id in &self.nodes {
            let node = self.global_network.get_node(&node_id);
            Self::move_node(
                &node,
                &mut self.links,
                &mut exited_vehicles,
                events,
                veh_def,
                now,
            );
        }

        exited_vehicles
    }

    fn move_node(
        node: &Node,
        links: &mut HashMap<usize, SimLink>,
        exited_vehicles: &mut Vec<ExitReason>,
        events: &mut EventsPublisher,
        veh_def: Option<&VehicleDefinitions>,
        now: u32,
    ) {
        for link_id in &node.in_links {
            let vehicles = match links.get_mut(&link_id.internal).unwrap() {
                SimLink::LocalLink(l) => l.pop_front(now),
                SimLink::SplitInLink(sl) => sl.local_link_mut().pop_front(now),
                SimLink::SplitOutLink(_) => panic!("No out link expected as in link of a node."),
            };
            for mut vehicle in vehicles {
                if vehicle.is_current_link_last() {
                    vehicle.advance_route_index();
                    exited_vehicles.push(ExitReason::FinishRoute(vehicle));
                } else {
                    if let Some(exit_reason) =
                        Self::move_vehicle(vehicle, veh_def, links, events, now)
                    {
                        exited_vehicles.push(exit_reason);
                    }
                }
            }
        }
    }

    fn move_vehicle(
        mut vehicle: Vehicle,
        veh_def: Option<&VehicleDefinitions>,
        links: &mut HashMap<usize, SimLink>,
        events: &mut EventsPublisher,
        now: u32,
    ) -> Option<ExitReason> {
        events.publish_event(
            now,
            &Event::new_link_leave(vehicle.curr_route_elem as u64, vehicle.id),
        );
        vehicle.advance_route_index();
        match links.get_mut(&(vehicle.curr_route_elem as usize)).unwrap() {
            SimLink::LocalLink(l) => {
                events.publish_event(now, &Event::new_link_enter(l.id() as u64, vehicle.id));
                l.push_vehicle(vehicle, now, veh_def);
                None
            }
            SimLink::SplitOutLink(_) => Some(ExitReason::ReachedBoundary(vehicle)),
            SimLink::SplitInLink(_) => {
                panic!("Not expecting to move a vehicle onto a split in link.")
            }
        }
    }
}
