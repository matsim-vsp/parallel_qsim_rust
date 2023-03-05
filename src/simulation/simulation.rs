use crate::simulation::config::Config;
use crate::simulation::messaging::events::proto::Event;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::message_broker::{MessageBroker, MpiMessageBroker};
use crate::simulation::messaging::messages::proto::leg::Route;
use crate::simulation::messaging::messages::proto::simulation_update_message::Type;
use crate::simulation::messaging::messages::proto::{
    Activity, Agent, GenericRoute, TrafficInfoMessage, Vehicle, VehicleMessage, VehicleType,
};
use crate::simulation::messaging::travel_time_collector::TravelTimeCollector;
use crate::simulation::network::link::Link;
use crate::simulation::network::network_partition::NetworkPartition;
use crate::simulation::network::node::{ExitReason, NodeVehicle};
use crate::simulation::population::Population;
use crate::simulation::routing::router::Router;
use crate::simulation::time_queue::TimeQueue;
use log::{debug, info};
use rust_road_router::algo::customizable_contraction_hierarchy::CCH;
use std::collections::{HashMap, HashSet};

pub struct Simulation<'sim> {
    activity_q: TimeQueue<Agent>,
    teleportation_q: TimeQueue<Vehicle>,
    network: NetworkPartition<Vehicle>,
    message_broker: MpiMessageBroker,
    events: EventsPublisher,
    router: Option<Router<'sim>>,
    cch: &'sim Option<CCH>,
}

impl<'sim> Simulation<'sim> {
    pub fn new(
        config: &Config,
        network: NetworkPartition<Vehicle>,
        population: Population,
        message_broker: MpiMessageBroker,
        events: EventsPublisher,
        router: Option<Router<'sim>>,
        cch: &'sim Option<CCH>,
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
            router,
            cch,
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

            if self.router.is_some() {
                let curr_act = agent.curr_act();
                let next_act = agent.next_act();

                let (route, travel_time) = self.find_route(curr_act, next_act);
                let dep_time = curr_act.end_time;

                agent.push_leg(
                    dep_time,
                    travel_time,
                    route,
                    curr_act.link_id,
                    next_act.link_id,
                );
            }

            //here, current element counter is going to be increased
            agent.advance_plan();

            assert_ne!(agent.curr_plan_elem % 2, 0);

            let leg = agent.curr_leg();

            match leg.route.as_ref().unwrap() {
                Route::GenericRoute(route) => {
                    self.events.publish_event(
                        now,
                        &Event::new_departure(agent_id, route.start_link, leg.mode.clone()),
                    );

                    if Simulation::is_local_route(route, &self.message_broker) {
                        let veh = Vehicle::new(agent.id, VehicleType::Teleported, agent);
                        self.teleportation_q.add(veh, now);
                    } else {
                        let veh = Vehicle::new(agent.id, VehicleType::Teleported, agent);
                        self.message_broker.add_veh(veh, now);
                    }
                }
                Route::NetworkRoute(route) => {
                    let link_id = route.route.get(0).unwrap();
                    self.events.publish_event(
                        now,
                        &Event::new_departure(agent_id, *link_id, leg.mode.clone()),
                    );

                    self.events.publish_event(
                        now,
                        &Event::new_person_enters_veh(agent_id, route.vehicle_id),
                    );

                    let veh = Vehicle::new(route.vehicle_id, VehicleType::Network, agent);
                    self.veh_onto_network(veh, true, now);
                }
            }
        }
    }

    fn find_route(&mut self, from_act: &Activity, to_act: &Activity) -> (Vec<u64>, Option<u32>) {
        let query_result = self
            .router
            .as_mut()
            .unwrap()
            .query_coordinates(from_act.x, from_act.y, to_act.x, to_act.y);

        let route = query_result.path.expect("There is no route!");
        let travel_time = query_result.travel_time;

        if route.is_empty() {
            debug!("Route between {:?} and {:?} is empty.", from_act, to_act);
        }

        (route, travel_time)
    }

    fn veh_onto_network(&mut self, vehicle: Vehicle, from_act: bool, now: u32) {
        let link_id = vehicle.curr_link_id().unwrap(); // in this case there should always be a link id.
        let link = self.network.links.get_mut(&link_id).expect(&*format!(
            "Cannot find link for link_id {:?} and vehicle {:?}",
            link_id, vehicle
        ));

        if !from_act {
            self.events
                .publish_event(now, &Event::new_link_enter(link_id as u64, vehicle.id));
        }
        match link {
            Link::LocalLink(link) => link.push_vehicle(vehicle, now),
            Link::SplitInLink(in_link) => {
                let local_link = in_link.local_link_mut();
                local_link.push_vehicle(vehicle, now)
            }
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
            let leg = agent.curr_leg();
            if let Route::GenericRoute(route) = &leg.route.as_ref().unwrap() {
                self.events.publish_event(
                    now,
                    &Event::new_travelled(agent.id, route.distance, leg.mode.clone()),
                );
            }
            agent.advance_plan();
            self.activity_q.add(agent, now);
        }
    }

    fn move_nodes(&mut self, now: u32) {
        for node in self.network.nodes.values() {
            let exited_vehicles =
                node.move_vehicles(&mut self.network.links, now, &mut self.events);

            for exit_reason in exited_vehicles {
                match exit_reason {
                    ExitReason::FinishRoute(vehicle) => {
                        let veh_id = vehicle.id;
                        let mut agent = vehicle.agent.unwrap();
                        let leg_mode = agent.curr_leg().mode.clone();

                        self.events
                            .publish_event(now, &Event::new_person_leaves_veh(agent.id, veh_id));

                        agent.advance_plan();
                        let act = agent.curr_act();

                        self.events.publish_event(
                            now,
                            &Event::new_arrival(agent.id, act.link_id, leg_mode),
                        );

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
    }

    fn send_receive(&mut self, now: u32) {
        // this might be configurable
        let traffic_update_interval_in_min = 15;

        let traffic_update =
            self.router.is_some() && now % (60 * traffic_update_interval_in_min) == 0;

        if traffic_update {
            debug!(
                "Process {}: Traffic update at {}",
                self.message_broker.rank, now
            );
            let collected_travel_times = self
                .events
                .get_subscriber::<TravelTimeCollector>()
                .map(|travel_time_collector| travel_time_collector.get_travel_times());

            self.message_broker.add_travel_times(
                self.get_travel_times_by_link_to_send(collected_travel_times.unwrap()),
            );

            self.events
                .get_subscriber::<TravelTimeCollector>()
                .unwrap()
                .flush();
        }

        let update_messages = self.message_broker.send_recv(now, traffic_update);

        let mut vehicle_update_messages = Vec::new();
        let mut traffic_info_messages = Vec::new();

        for update in update_messages {
            if let Some(message_type) = update.r#type {
                match message_type {
                    Type::VehicleMessage(message) => vehicle_update_messages.push(message),
                    Type::TrafficInfoMessage(message) => traffic_info_messages.push(message),
                }
            } else {
                panic!("The SimulationUpdateMessage is expected to be either a VehicleMessage or a TrafficInfoMessage.");
            }
        }

        self.handle_vehicle_messages(now, vehicle_update_messages);
        self.handle_traffic_info_messages(traffic_info_messages);
    }

    fn get_travel_times_by_link_to_send(
        &self,
        collected_travel_times: HashMap<u64, u32>,
    ) -> HashMap<u64, u32> {
        let mut result = HashMap::new();

        let link_ids_of_process = self
            .network
            .links
            .iter()
            .filter(|(id, link)| match link {
                Link::LocalLink(_) => true,
                Link::SplitInLink(_) => true,
                Link::SplitOutLink(_) => false,
            })
            .map(|(id, _)| *id as u64)
            .collect::<HashSet<u64>>();

        // for each collected travel time: add if currently known travel time is different
        for (id, travel_time) in &collected_travel_times {
            if *travel_time != self.get_router_ref().get_current_travel_time(*id) {
                result.insert(*id, *travel_time);
            } else {
                debug!(
                    "Process {:?} | (link {:?}, travel time: {:?}) was already there.",
                    self.message_broker.rank, id, travel_time
                );
            }
        }

        // for each link about which no travel time was collected: add initial travel time if currently known travel time is different
        for id in link_ids_of_process
            .difference(&collected_travel_times.into_keys().collect::<HashSet<u64>>())
        {
            let initial_travel_time = self.get_router_ref().get_initial_travel_time(*id);
            if self.get_router_ref().get_current_travel_time(*id) != initial_travel_time {
                result.insert(*id, initial_travel_time);
            }
        }
        if !result.is_empty() {
            debug!("Traffic update to be sent: {:?}", result);
        }
        result
    }

    fn handle_vehicle_messages(&mut self, now: u32, vehicle_update_messages: Vec<VehicleMessage>) {
        let vehicles = vehicle_update_messages
            .into_iter()
            .flat_map(|msg| msg.vehicles)
            .collect::<Vec<Vehicle>>();

        for vehicle in vehicles {
            match vehicle.r#type() {
                VehicleType::Teleported => {
                    self.teleportation_q.add(vehicle, now);
                }
                VehicleType::Network => {
                    self.veh_onto_network(vehicle, false, now);
                }
            }
        }
    }

    fn handle_traffic_info_messages(&mut self, traffic_info_messages: Vec<TrafficInfoMessage>) {
        if self.router.is_none() {
            return;
        }

        if traffic_info_messages.is_empty() {
            return;
        }
        debug!(
            "Processing traffic info messages: {:?}.",
            traffic_info_messages
        );

        let travel_times_by_link = traffic_info_messages
            .iter()
            .map(|info| &info.travel_times_by_link_id)
            .fold(HashMap::new(), |result, value| {
                result.into_iter().chain(value).collect()
            });

        let number_of_links_with_traffic_info = traffic_info_messages
            .iter()
            .map(|info| info.travel_times_by_link_id.len())
            .sum::<usize>();

        assert_eq!(
            number_of_links_with_traffic_info,
            travel_times_by_link.len()
        );

        let router = self.router.as_mut().unwrap();
        let network_with_new_travel_times = router
            .current_network
            .clone_with_new_travel_times_by_link(travel_times_by_link);

        debug!("There are travel time changes. Router will be customized.");
        router.customize(self.cch.as_ref().unwrap(), network_with_new_travel_times);
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

    fn get_router_ref(&self) -> &Router {
        self.router.as_ref().unwrap()
    }
}
