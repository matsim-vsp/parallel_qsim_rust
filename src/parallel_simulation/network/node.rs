use crate::parallel_simulation::events::Events;
use crate::parallel_simulation::network::link::Link;
use crate::parallel_simulation::vehicles::Vehicle;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Node {
    id: usize,
    in_links: Vec<usize>,
    out_links: Vec<usize>,
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
            if let Link::LocalLink(in_link) = links.get_mut(in_link_index).unwrap() {
                for mut vehicle in in_link.pop_front(now) {
                    //let in_link_id = in_link.id;
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
            Link::SplitLink(_) => exited_vehicles.push(ExitReason::ReachedBoundary(vehicle)),
        }
    }
}
