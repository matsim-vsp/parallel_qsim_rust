use std::sync::Arc;

use tracing::info;

use crate::simulation::config::{Config, RoutingMode};
use crate::simulation::messaging::events::proto::Event;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::{Agent, Vehicle};
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::population::population::Population;
use crate::simulation::replanning::replanner::Replanner;
use crate::simulation::time_queue::TimeQueue;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::vehicles::vehicle_type::LevelOfDetail;

pub struct Simulation<C>
where
    C: SimCommunicator,
{
    activity_q: TimeQueue<Agent>,
    teleportation_q: TimeQueue<Vehicle>,
    network: SimNetworkPartition,
    garage: Garage,
    net_message_broker: NetMessageBroker<C>,
    events: EventsPublisher,
    replanner: Box<dyn Replanner>,
}

impl<C> Simulation<C>
where
    C: SimCommunicator + 'static,
{
    pub fn new(
        config: Arc<Config>,
        network: SimNetworkPartition,
        garage: Garage,
        mut population: Population,
        net_message_broker: NetMessageBroker<C>,
        events: EventsPublisher,
        replanner: Box<dyn Replanner>,
    ) -> Self {
        let mut activity_q = TimeQueue::new();

        // take agents and copy them into queues. This way we can keep population around to translate
        // ids for events processing...
        let agents = std::mem::take(&mut population.agents);

        for agent in agents.into_values() {
            activity_q.add(agent, config.start_time);
        }

        Simulation {
            network,
            garage,
            teleportation_q: TimeQueue::new(),
            activity_q,
            net_message_broker,
            events,
            replanner,
        }
    }

    pub fn run(&mut self, start_time: u32, end_time: u32) {
        // use fixed start and end times
        let mut now = start_time;
        info!(
            "Starting #{}. Network neighbors: {:?}, Start time {start_time}, End time {end_time}",
            self.net_message_broker.rank(),
            self.network.neighbors(),
        );

        while now <= end_time {
            if self.net_message_broker.rank() == 0 && now % 1800 == 0 {
                //if now % 600 == 0 {
                //if now % 800 == 0 {
                let _hour = now / 3600;
                let _min = (now % 3600) / 60;
                info!(
                    "#{} of Qsim at {_hour}:{_min}",
                    self.net_message_broker.rank()
                );
            }
            self.wakeup(now);
            self.terminate_teleportation(now);
            self.move_nodes(now);
            self.move_links(now);

            self.replanner.next_time_step(now, &mut self.events);

            now += 1;
        }

        // maybe this belongs into the controller? Then this would have to be a &mut instead of owned.
        self.events.finish();
    }

    fn wakeup(&mut self, now: u32) {
        let agents = self.activity_q.pop(now);

        for mut agent in agents {
            self.update_agent(&mut agent, now);

            let act_type: Id<String> = Id::get(agent.curr_act().act_type);
            self.events.publish_event(
                now,
                &Event::new_act_end(
                    agent.id,
                    agent.curr_act().link_id,
                    act_type.external().to_string(),
                ),
            );

            let mut vehicle = self.departure(agent, now);
            let veh_type_id = Id::get(vehicle.r#type);
            let veh_type = self.garage.vehicle_types.get(&veh_type_id).unwrap();

            match veh_type.lod {
                LevelOfDetail::Network => {
                    self.events.publish_event(
                        now,
                        &Event::new_person_enters_veh(vehicle.agent().id, vehicle.id),
                    );
                    self.network.send_veh_en_route(vehicle, now);
                }
                LevelOfDetail::Teleported => {
                    if Simulation::is_local_route(&vehicle, &self.net_message_broker) {
                        self.teleportation_q.add(vehicle, now);
                    } else {
                        // we need to call advance here, so that the vehicle's current link index
                        // points to the end link of the route array.
                        vehicle.advance_route_index();
                        self.net_message_broker.add_veh(vehicle, now);
                    }
                }
            }
        }
    }

    fn departure(&mut self, mut agent: Agent, now: u32) -> Vehicle {
        //here, current element counter is going to be increased
        agent.advance_plan();

        assert_ne!(agent.curr_plan_elem % 2, 0);

        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let leg_mode: Id<String> = Id::get(leg.mode);
        self.events.publish_event(
            now,
            &Event::new_departure(
                agent.id,
                route.start_link(),
                leg_mode.external().to_string(),
            ),
        );

        let veh_id = Id::get(route.veh_id);
        self.garage.unpark_veh(agent, &veh_id)
    }

    fn update_agent(&mut self, agent: &mut Agent, now: u32) {
        let agent_id = self.population.agent_ids.get(agent.id);
        self.replanner.update_agent(
            now,
            agent,
            &agent_id,
            &self.population.act_types,
            self.network.global_network,
            &self.garage,
        )
    }

    fn terminate_teleportation(&mut self, now: u32) {
        let teleportation_vehicles = self.teleportation_q.pop(now);
        for vehicle in teleportation_vehicles {
            // park the vehice - get the agent out of the vehicle
            let mut agent = self.garage.park_veh(vehicle);

            // emmit travelled
            let leg = agent.curr_leg();
            let route = leg.route.as_ref().unwrap();
            let mode: Id<String> = Id::get(leg.mode);
            self.events.publish_event(
                now,
                &Event::new_travelled(agent.id, route.distance, mode.external().to_string()),
            );

            // emmit arrival
            self.events.publish_event(
                now,
                &Event::new_arrival(agent.id, route.end_link(), mode.external().to_string()),
            );

            // advance plan to activity and put agent into activity q.
            agent.advance_plan();

            // emmit act start event
            let act = agent.curr_act();
            let act_type: Id<String> = Id::get(act.act_type);
            self.events.publish_event(
                now,
                &Event::new_act_start(agent.id, act.link_id, act_type.external().to_string()),
            );
            self.activity_q.add(agent, now);
        }
    }

    fn move_nodes(&mut self, now: u32) {
        let exited_vehicles = self.network.move_nodes(&mut self.events, now);

        for veh in exited_vehicles {
            self.events
                .publish_event(now, &Event::new_person_leaves_veh(veh.agent().id, veh.id));
            let veh_type_id = Id::get(veh.r#type);
            let veh_type = self.garage.vehicle_types.get(&veh_type_id).unwrap();
            let mode = veh_type.net_mode.external().to_string();
            let mut agent = self.garage.park_veh(veh);

            // move to next activity
            agent.advance_plan();
            let act = agent.curr_act();
            self.events
                .publish_event(now, &Event::new_arrival(agent.id, act.link_id, mode));
            let act_type: Id<String> = Id::get(act.act_type);
            self.events.publish_event(
                now,
                &Event::new_act_start(agent.id, act.link_id, act_type.external().to_string()),
            );
            self.activity_q.add(agent, now);
        }
    }

    fn move_links(&mut self, now: u32) {
        let (vehicles, storage_cap) = self.network.move_links(now);

        for veh in vehicles {
            self.net_message_broker.add_veh(veh, now);
        }

        for cap in storage_cap {
            self.net_message_broker.add_cap(cap, now);
        }

        let sync_messages = self.net_message_broker.send_recv(now);

        for msg in sync_messages {
            self.network.update_storage_caps(msg.storage_capacities);

            for veh in msg.vehicles {
                let veh_type_id = Id::get(veh.r#type);
                let veh_type = self.garage.vehicle_types.get(&veh_type_id).unwrap();
                match veh_type.lod {
                    LevelOfDetail::Network => self.network.send_veh_en_route(veh, now),
                    LevelOfDetail::Teleported => self.teleportation_q.add(veh, now),
                }
            }
        }
    }

    fn is_local_route(veh: &Vehicle, message_broker: &NetMessageBroker<C>) -> bool {
        let leg = veh.agent.as_ref().unwrap().curr_leg();
        let route = leg.route.as_ref().unwrap();
        let from = message_broker.rank_for_link(route.start_link());
        let to = message_broker.rank_for_link(route.end_link());
        from == to
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::Arc;
    use std::thread;
    use std::thread::JoinHandle;

    use nohash_hasher::IntMap;
    use tracing::info;

    use crate::simulation::config::Config;
    use crate::simulation::messaging::communication::communicators::{
        ChannelSimCommunicator, DummySimCommunicator, SimCommunicator,
    };
    use crate::simulation::messaging::communication::message_broker::NetMessageBroker;
    use crate::simulation::messaging::events::proto::Event;
    use crate::simulation::messaging::events::{EventsPublisher, EventsSubscriber};
    use crate::simulation::network::global_network::Network;
    use crate::simulation::network::sim_network::SimNetworkPartition;
    use crate::simulation::population::population::Population;
    use crate::simulation::replanning::replanner::{
        DummyReplanner, ReRouteTripReplanner, Replanner,
    };
    use crate::simulation::replanning::routing::travel_time_collector::TravelTimeCollector;
    use crate::simulation::simulation::Simulation;
    use crate::simulation::vehicles::garage::Garage;

    #[test]
    fn execute_3_links_single_part() {
        let config = Arc::new(
            Config::builder()
                .network_file(String::from("./assets/3-links/3-links-network.xml"))
                .population_file(String::from("./assets/3-links/1-agent-full-leg.xml"))
                .vehicles_file(String::from("./assets/3-links/vehicles.xml"))
                .output_dir(String::from(
                    "./test_output/simulation/execute_3_links_single_part",
                ))
                .build(),
        );

        execute_sim(
            DummySimCommunicator(),
            Box::new(TestSubscriber::new()),
            config,
        );
    }

    #[test]
    fn execute_3_links_2_parts() {
        let config = Arc::new(
            Config::builder()
                .network_file(String::from("./assets/3-links/3-links-network.xml"))
                .population_file(String::from("./assets/3-links/1-agent-full-leg.xml"))
                .vehicles_file(String::from("./assets/3-links/vehicles.xml"))
                .output_dir(String::from(
                    "./test_output/simulation/execute_3_links_2_parts",
                ))
                .num_parts(2)
                .partition_method(String::from("none"))
                .build(),
        );

        execute_sim_with_channels(config, None);
    }

    #[test]
    fn execute_adhoc_routing_one_part_no_updates() {
        let config = Arc::new(
            Config::builder()
                .network_file(String::from(
                    "./assets/adhoc_routing/no_updates/network.xml",
                ))
                .population_file(String::from("./assets/adhoc_routing/no_updates/agents.xml"))
                .vehicles_file(String::from("./assets/adhoc_routing/vehicles.xml"))
                .output_dir(String::from(
                    "./test_output/simulation/adhoc_routing_one_part",
                ))
                .routing_mode(RoutingMode::AdHoc)
                .num_parts(1)
                .partition_method(String::from("none"))
                .build(),
        );

        execute_sim(
            DummySimCommunicator(),
            Box::new(TestSubscriber::new_with_events_from_file(
                "./assets/adhoc_routing/no_updates/expected_events.pbf",
            )),
            config,
        );
    }

    #[test]
    fn execute_adhoc_routing_two_parts_no_updates() {
        let config = Arc::new(
            Config::builder()
                .network_file(String::from(
                    "./assets/adhoc_routing/no_updates/network.xml",
                ))
                .population_file(String::from("./assets/adhoc_routing/no_updates/agents.xml"))
                .vehicles_file(String::from("./assets/adhoc_routing/vehicles.xml"))
                .output_dir(String::from(
                    "./test_output/simulation/adhoc_routing_two_parts",
                ))
                .routing_mode(RoutingMode::AdHoc)
                .num_parts(2)
                .partition_method(String::from("none"))
                .build(),
        );

        execute_sim_with_channels(
            config,
            Some("./assets/adhoc_routing/no_updates/expected_events.pbf"),
        );
    }

    #[test]
    fn execute_adhoc_routing_one_part_with_updates() {
        let config = Arc::new(
            Config::builder()
                .network_file(String::from(
                    "./assets/adhoc_routing/with_updates/network.xml",
                ))
                .population_file(String::from(
                    "./assets/adhoc_routing/with_updates/agents.xml",
                ))
                .vehicles_file(String::from("./assets/adhoc_routing/vehicles.xml"))
                .output_dir(String::from(
                    "./test_output/simulation/adhoc_routing_one_part",
                ))
                .routing_mode(RoutingMode::AdHoc)
                .num_parts(1)
                .partition_method(String::from("none"))
                .build(),
        );

        execute_sim(
            DummySimCommunicator(),
            Box::new(TestSubscriber::new_with_events_from_file(
                "./assets/adhoc_routing/with_updates/expected_events.pbf",
            )),
            config,
        );
    }

    #[test]
    fn execute_adhoc_routing_two_parts_with_updates() {
        let config = Arc::new(
            Config::builder()
                .network_file(String::from(
                    "./assets/adhoc_routing/with_updates/network.xml",
                ))
                .population_file(String::from(
                    "./assets/adhoc_routing/with_updates/agents.xml",
                ))
                .vehicles_file(String::from("./assets/adhoc_routing/vehicles.xml"))
                .output_dir(String::from(
                    "./test_output/simulation/adhoc_routing_two_parts",
                ))
                .routing_mode(RoutingMode::AdHoc)
                .num_parts(2)
                .partition_method(String::from("none"))
                .build(),
        );

        execute_sim_with_channels(
            config,
            Some("./assets/adhoc_routing/with_updates/expected_events.pbf"),
        );
    }

    fn execute_sim_with_channels(config: Arc<Config>, expected_events_file: Option<&str>) {
        let comms = ChannelSimCommunicator::create_n_2_n(config.num_parts);
        let mut receiver = match expected_events_file {
            None => ReceivingSubscriber::new(),
            Some(e) => ReceivingSubscriber::new_with_events_from_file(e),
        };

        let mut handles: IntMap<u32, JoinHandle<()>> = comms
            .into_iter()
            .map(|comm| {
                let config = config.clone();
                let subscr = SendingSubscriber {
                    rank: comm.rank(),
                    sender: receiver.channel.0.clone(),
                };
                (
                    comm.rank(),
                    thread::spawn(move || execute_sim(comm, Box::new(subscr), config)),
                )
            })
            .collect();

        // create another thread for the receiver, so that the main thread doesn't block.
        let receiver_handle = thread::spawn(move || receiver.start_listen());
        handles.insert(handles.len() as u32, receiver_handle);

        try_join(handles);
    }

    #[test]
    fn test_rvr_scenario() {
        let config = Arc::new(
            Config::builder()
                .network_file(String::from(
                    "/Users/janek/Documents/rust_q_sim/input/rvr.network.xml.gz",
                ))
                .population_file(String::from(
                    "/Users/janek/Documents/rust_q_sim/input/rvr.1pct.plans.xml.gz",
                ))
                .vehicles_file(String::from(
                    "/Users/janek/Documents/rust_q_sim/input/rvr.vehicles.xml",
                ))
                .output_dir(String::from("/Users/janek/Documents/rust_q_sim/output-wip"))
                .num_parts(1)
                .partition_method(String::from("none"))
                .build(),
        );

        let _guards = logging::init_logging(config.output_dir.as_ref(), 0.to_string());

        execute_sim(DummyNetCommunicator(), Box::new(EmtpySubscriber {}), config)
    }

    fn execute_sim<C: SimCommunicator + 'static>(
        comm: C,
        test_subscriber: Box<dyn EventsSubscriber + Send>,
        config: Arc<Config>,
    ) {
        let net = Network::from_file(
            &config.network_file,
            config.num_parts,
            &config.partition_method,
        );
        let mut garage = Garage::from_file(&config.vehicles_file);
        let pop = Population::from_file(&config.population_file, &net, &mut garage, comm.rank());
        let sim_net = SimNetworkPartition::from_network(&net, comm.rank(), config.sample_size);

        let id_part: Vec<_> = net
            .links
            .iter()
            .map(|l| (l.id.external(), l.partition))
            .collect();

        info!("#{} {id_part:?}", comm.rank());

        let mut events = EventsPublisher::new();
        events.add_subscriber(test_subscriber);
        events.add_subscriber(Box::new(TravelTimeCollector::new()));

        let rc = Rc::new(comm);
        let broker = NetMessageBroker::new(rc.clone(), &sim_net);

        let replanner: Box<dyn Replanner> = if config.routing_mode == RoutingMode::AdHoc {
            Box::new(ReRouteTripReplanner::new(&sim_net, &garage, rc))
        } else {
            Box::new(DummyReplanner {})
        };

        let mut sim = Simulation::new(
            config.clone(),
            sim_net,
            garage,
            pop,
            broker,
            events,
            replanner,
        );

        sim.run(config.start_time, config.end_time);
    }

    /// Have this more complicated join logic, so that threads in the back of the handle vec can also
    /// cause the main thread to panic.
    fn try_join(mut handles: IntMap<u32, JoinHandle<()>>) {
        while !handles.is_empty() {
            let mut finished = Vec::new();
            for (i, handle) in handles.iter() {
                if handle.is_finished() {
                    finished.push(*i);
                }
            }
            for i in finished {
                let handle = handles.remove(&i).unwrap();
                handle.join().expect("Error in a thread");
            }
        }
    }

    struct EmtpySubscriber {}

    impl EventsSubscriber for EmtpySubscriber {
        fn receive_event(&mut self, _time: u32, _event: &Event) {
            // nothing.
        }

        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }

    struct TestSubscriber {
        next_index: usize,
        expected_events: Vec<(u32, Event)>,
    }

    struct ReceivingSubscriber {
        test_subscriber: TestSubscriber,
        channel: (Sender<(u32, Event)>, Receiver<(u32, Event)>),
    }

    struct SendingSubscriber {
        #[allow(dead_code)]
        rank: u32,
        sender: Sender<(u32, Event)>,
    }

    impl EventsSubscriber for SendingSubscriber {
        fn receive_event(&mut self, time: u32, event: &Event) {
            self.sender
                .send((time, event.clone()))
                .expect("Failed on sending event message!");
        }

        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl ReceivingSubscriber {
        fn new() -> Self {
            Self {
                test_subscriber: TestSubscriber::new(),
                channel: channel(),
            }
        }

        fn new_with_events_from_file(events_file: &str) -> Self {
            Self {
                test_subscriber: TestSubscriber::new_with_events_from_file(events_file),
                channel: channel(),
            }
        }

        fn start_listen(&mut self) {
            while self.test_subscriber.next_index < self.test_subscriber.expected_events.len() {
                let (time, event) = self
                    .channel
                    .1
                    .recv()
                    .expect("Something went wrong while listening for events");
                self.test_subscriber.receive_event(time, &event);
            }
        }
    }

    impl TestSubscriber {
        fn new() -> Self {
            Self {
                next_index: 0,
                expected_events: Self::expected_events(),
            }
        }

        fn new_with_events_from_file(events_file: &str) -> Self {
            Self {
                next_index: 0,
                expected_events: Self::expected_events_from_file(events_file),
            }
        }

        fn expected_events_from_file(events_file: &str) -> Vec<(u32, Event)> {
            let reader = EventsReader::from_file(&PathBuf::from(events_file));
            let mut result = Vec::new();
            for (time, events) in reader {
                for event in events {
                    result.push((time, event));
                }
            }
            result
        }

        fn expected_events() -> Vec<(u32, Event)> {
            let result = vec![
                (32400, Event::new_act_end(0, 0, String::from("home"))),
                (32400, Event::new_departure(0, 0, String::from("walk"))),
                (32408, Event::new_travelled(0, 10., String::from("walk"))),
                (32408, Event::new_arrival(0, 0, String::from("walk"))),
                (
                    32408,
                    Event::new_act_start(0, 0, String::from("car interaction")),
                ),
                (
                    32409,
                    Event::new_act_end(0, 0, String::from("car interaction")),
                ),
                (32409, Event::new_departure(0, 0, String::from("car"))),
                (32409, Event::new_person_enters_veh(0, 0)),
                // skip vehicle enters traffic
                (32419, Event::new_link_leave(0, 0)),
                (32419, Event::new_link_enter(1, 0)),
                (32519, Event::new_link_leave(1, 0)),
                (32519, Event::new_link_enter(2, 0)),
                (32529, Event::new_person_leaves_veh(0, 0)),
                (32529, Event::new_arrival(0, 2, String::from("car"))),
                (
                    32529,
                    Event::new_act_start(0, 2, String::from("car interaction")),
                ),
                (
                    32530,
                    Event::new_act_end(0, 2, String::from("car interaction")),
                ),
                (32530, Event::new_departure(0, 2, String::from("walk"))),
                (32546, Event::new_travelled(0, 20., String::from("walk"))),
                (32546, Event::new_arrival(0, 2, String::from("walk"))),
                (32546, Event::new_act_start(0, 2, String::from("errands"))),
            ];

            result
        }
    }

    impl EventsSubscriber for TestSubscriber {
        fn receive_event(&mut self, time: u32, event: &Event) {
            let (expected_time, expected_event) =
                self.expected_events.get(self.next_index).unwrap();
            self.next_index += 1;
            assert_eq!(expected_time, &time);
            assert_eq!(expected_event, event, "Unexpected Event at time {}", time);
        }

        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }
}
