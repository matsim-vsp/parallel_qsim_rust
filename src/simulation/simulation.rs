use tracing::info;

use crate::simulation::config::Config;
use crate::simulation::messaging::events::proto::Event;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::message_broker::{MessageBroker, MpiMessageBroker};
use crate::simulation::messaging::messages::proto::leg::Route;
use crate::simulation::messaging::messages::proto::{Agent, GenericRoute, Vehicle};
use crate::simulation::network::link::SimLink;
use crate::simulation::network::sim_network::{ExitReason, SimNetworkPartition};
use crate::simulation::time_queue::TimeQueue;

pub struct Simulation<'sim> {
    activity_q: TimeQueue<Agent>,
    teleportation_q: TimeQueue<Vehicle>,
    network: SimNetworkPartition<'sim>,
    message_broker: MpiMessageBroker,
    events: EventsPublisher,
}

impl<'sim> Simulation<'sim> {
    pub fn new(
        config: &Config,
        network: SimNetworkPartition<'sim>,
        population: crate::simulation::population::population::Population,
        message_broker: MpiMessageBroker,
        events: EventsPublisher,
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
            events,
        }
    }

    pub fn run(&mut self, start_time: u32, end_time: u32) {
        // use fixed start and end times
        let mut now = start_time;
        info!(
            "Starting #{}. Network neighbors: {:?}, Start time {start_time}, End time {end_time}",
            self.message_broker.rank,
            self.network.neighbors(),
        );

        while now <= end_time {
            if self.message_broker.rank == 0 && now % 600 == 0 {
                let _hour = now / 3600;
                let _min = (now % 3600) / 60;
                info!("#{} of Qsim at {_hour}:{_min}", self.message_broker.rank);
            }
            self.wakeup(now);
            self.terminate_teleportation(now);
            self.move_nodes(now);
            self.send_receive(now);

            now += 1;
        }

        // maybe this belongs into the controller? Then this would have to be a &mut instead of owned.
        self.events.finish();
    }

    fn wakeup(&mut self, now: u32) {
        let agents = self.activity_q.pop(now);

        for mut agent in agents {
            let agent_id = agent.id;
            self.events.publish_event(
                now,
                &Event::new_act_end(
                    agent_id,
                    agent.curr_act().link_id,
                    agent.curr_act().act_type.clone(),
                ),
            );

            //here, current element counter is going to be increased
            agent.advance_plan();

            assert_ne!(agent.curr_plan_elem % 2, 0);

            let leg = agent.curr_leg();

            match leg.route.as_ref().unwrap() {
                Route::GenericRoute(route) => {
                    self.events.publish_event(
                        now,
                        &Event::new_departure(agent_id, route.start_link, String::from("")),
                    );

                    if Simulation::is_local_route(route, &self.message_broker) {
                        let veh = Vehicle::new(agent.id, 1, leg.mode.clone(), agent);
                        self.teleportation_q.add(veh, now);
                    } else {
                        let veh = Vehicle::new(agent.id, 1, leg.mode.clone(), agent);
                        self.message_broker.add_veh(veh, now);
                    }
                }
                Route::NetworkRoute(route) => {
                    let link_id = route.route.first().unwrap();
                    self.events.publish_event(
                        now,
                        &Event::new_departure(agent_id, *link_id, String::from("")),
                    );

                    self.events.publish_event(
                        now,
                        &Event::new_person_enters_veh(agent_id, route.vehicle_id),
                    );

                    let veh = Vehicle::new(route.vehicle_id, 0, leg.mode.clone(), agent);
                    self.veh_onto_network(veh, true, now);
                }
            }
        }
    }

    fn veh_onto_network(&mut self, vehicle: Vehicle, from_act: bool, now: u32) {
        let link_id_internal = vehicle.curr_link_id().unwrap(); // in this case there should always be a link id.
        let link_id = self.network.global_network.link_ids.get(link_id_internal);
        let link = self.network.links.get_mut(&link_id).unwrap_or_else(|| {
            panic!(
                "Cannot find link for link_id {:?} and vehicle {:?}",
                link_id, vehicle
            )
        });

        if !from_act {
            self.events.publish_event(
                now,
                &Event::new_link_enter(link_id.internal as u64, vehicle.id),
            );
        }
        match link {
            SimLink::Local(link) => link.push_vehicle(vehicle, now),
            SimLink::In(in_link) => {
                let local_link = in_link.local_link_mut();
                local_link.push_vehicle(vehicle, now)
            }
            SimLink::Out(_) => {
                panic!("Vehicles should not start on out links...")
            }
        }
    }

    fn terminate_teleportation(&mut self, now: u32) {
        let teleportation_vehicles = self.teleportation_q.pop(now);
        for vehicle in teleportation_vehicles {
            // handle travelled
            let mut agent = vehicle.agent.unwrap();
            let leg = agent.curr_leg();
            if let Route::GenericRoute(route) = &leg.route.as_ref().unwrap() {
                self.events.publish_event(
                    now,
                    &Event::new_travelled(agent.id, route.distance, String::from("")),
                );
            }
            agent.advance_plan();
            self.activity_q.add(agent, now);
        }
    }

    fn move_nodes(&mut self, now: u32) {
        let exited_vehicles = self.network.move_nodes(&mut self.events, now);

        for exit_reason in exited_vehicles {
            match exit_reason {
                ExitReason::FinishRoute(vehicle) => {
                    let veh_id = vehicle.id;
                    let mut agent = vehicle.agent.unwrap();
                    let leg_mode = 0; // todo fix mode

                    self.events
                        .publish_event(now, &Event::new_person_leaves_veh(agent.id, veh_id));

                    agent.advance_plan();
                    let act = agent.curr_act();

                    self.events.publish_event(
                        now,
                        &Event::new_arrival(agent.id, act.link_id, String::from("some mode")),
                    ); //todo fix  mode

                    self.events.publish_event(
                        now,
                        &Event::new_act_start(agent.id, act.link_id, act.act_type.clone()),
                    );
                    self.activity_q.add(agent, now);
                }
                ExitReason::ReachedBoundary(vehicle) => {
                    self.message_broker.add_veh(vehicle, now);
                }
            }
        }
    }

    fn send_receive(&mut self, now: u32) {
        let vehicle_update_messages = self.message_broker.send_recv(now);

        let vehicles = vehicle_update_messages
            .into_iter()
            .flat_map(|msg| msg.vehicles)
            .collect::<Vec<Vehicle>>();

        for vehicle in vehicles {
            match vehicle.r#type {
                1 => {
                    self.teleportation_q.add(vehicle, now);
                }
                0 => {
                    self.veh_onto_network(vehicle, false, now);
                }
                _ => {}
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
