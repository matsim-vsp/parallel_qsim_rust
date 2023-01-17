use crate::config::Config;
use crate::mpi::message_broker::{MessageBroker, MpiMessageBroker};
use crate::mpi::messages::proto::leg::Route;
use crate::mpi::messages::proto::{Agent, GenericRoute, Vehicle, VehicleType};
use crate::mpi::population::Population;
use crate::mpi::time_queue::TimeQueue;
use crate::parallel_simulation::network::link::Link;
use crate::parallel_simulation::network::network_partition::NetworkPartition;
use crate::parallel_simulation::network::node::{ExitReason, NodeVehicle};
use log::info;

pub struct Simulation {
    activity_q: TimeQueue<Agent>,
    teleportation_q: TimeQueue<Vehicle>,
    network: NetworkPartition<Vehicle>,
    message_broker: MpiMessageBroker,
}

impl Simulation {
    pub fn new(
        config: &Config,
        network: NetworkPartition<Vehicle>,
        population: Population,
        message_broker: MpiMessageBroker,
    ) -> Self {
        let mut activity_q = TimeQueue::new();
        for agent in population.agents.into_values() {
            activity_q.add(agent, config.start_time);
        }

        Simulation {
            network,
            teleportation_q: TimeQueue::new(),
            activity_q,
            message_broker,
        }
    }

    pub fn run(&mut self, start_time: u32, end_time: u32) {
        // use fixed start and end times
        let mut now = start_time;
        info!(
            "Starting #{}. Network neighbors: {:?}",
            self.message_broker.rank,
            self.network.neighbors(),
        );

        while now <= end_time {
            self.wakeup(now);
            self.terminate_teleportation(now);
            self.move_nodes(now);
            self.send(now);
            //self.events.flush();
            self.receive(now);
            now += 1;
        }
    }
    fn wakeup(&mut self, now: u32) {
        let agents = self.activity_q.pop(now);
        for mut agent in agents {
            // ACTEND EVENT here
            agent.advance_plan();
            //DEPARTURE EVENT here
            assert!(agent.curr_plan_elem % 2 != 0);

            let leg = agent.curr_leg();
            match leg.route.as_ref().unwrap() {
                Route::GenericRoute(route) => {
                    if Simulation::is_local_route(&route, &self.message_broker) {
                        let veh = Vehicle::new(agent.id, VehicleType::Teleported, agent);
                        self.teleportation_q.add(veh, now);
                    } else {
                        let veh = Vehicle::new(agent.id, VehicleType::Teleported, agent);
                        self.message_broker.add_veh(veh, now);
                    }
                }
                Route::NetworkRoute(route) => {
                    let veh = Vehicle::new(route.vehicle_id, VehicleType::Network, agent);
                    self.veh_onto_network(veh, now);
                }
            }
        }
    }

    fn veh_onto_network(&mut self, vehicle: Vehicle, now: u32) {
        let link_id = vehicle.curr_link_id().unwrap(); // in this case there should always be a link id.
        let link = self.network.links.get_mut(&link_id).unwrap();

        match link {
            Link::LocalLink(link) => link.push_vehicle(vehicle, now),
            Link::SplitInLink(in_link) => in_link.local_link_mut().push_vehicle(vehicle, now),
            Link::SplitOutLink(_) => {
                panic!("Vehicles should not start on out links...")
            }
        }
    }

    fn terminate_teleportation(&mut self, now: u32) {
        let teleportation_vehicles = self.teleportation_q.pop(now);
        for vehicle in teleportation_vehicles {
            // handle travelled
            let mut agent = vehicle.agent.unwrap();
            agent.advance_plan();
            self.activity_q.add(agent, now);
        }
    }
    fn move_nodes(&mut self, now: u32) {
        for node in self.network.nodes.values() {
            let exited_vehicles = node.move_vehicles(&mut self.network.links, now);

            for exit_reason in exited_vehicles {
                match exit_reason {
                    ExitReason::FinishRoute(vehicle) => {
                        let mut agent = vehicle.agent.unwrap();
                        // person leaves vehicle event
                        // arrival event
                        agent.advance_plan();
                        // act start event
                        self.activity_q.add(agent, now);
                    }
                    ExitReason::ReachedBoundary(vehicle) => {
                        self.message_broker.add_veh(vehicle, now);
                    }
                }
            }
        }
    }
    fn send(&mut self, now: u32) {
        self.message_broker.send(now);
    }
    fn receive(&mut self, now: u32) {
        let vehicles = self.message_broker.receive();
        for vehicle in vehicles {
            match vehicle.r#type() {
                VehicleType::Teleported => {
                    self.teleportation_q.add(vehicle, now);
                }
                VehicleType::Network => {
                    self.veh_onto_network(vehicle, now);
                }
            }
        }
    }

    fn is_local_route(route: &GenericRoute, message_broker: &MpiMessageBroker) -> bool {
        let (from, to) = Simulation::process_ids_for_generic_route(route, message_broker);
        from == to
    }
    fn process_ids_for_generic_route(
        route: &GenericRoute,
        message_broker: &MpiMessageBroker,
    ) -> (u64, u64) {
        let from_rank = message_broker.rank_for_link(route.start_link);
        let to_rank = message_broker.rank_for_link(route.end_link);
        (from_rank, to_rank)
    }
}
