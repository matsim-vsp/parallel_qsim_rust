use std::sync::Arc;

use tracing::info;

use crate::simulation::config::Config;
use crate::simulation::id::Id;
use crate::simulation::messaging::events::proto::Event;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::message_broker::{NetCommunicator, NetMessageBroker};
use crate::simulation::messaging::messages::proto::{Agent, Vehicle};
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::population::population::Population;
use crate::simulation::time_queue::TimeQueue;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::vehicles::vehicle_type::LevelOfDetail;

pub struct Simulation<C>
where
    C: NetCommunicator,
{
    activity_q: TimeQueue<Agent>,
    teleportation_q: TimeQueue<Vehicle>,
    network: SimNetworkPartition,
    garage: Garage,
    message_broker: NetMessageBroker<C>,
    events: EventsPublisher,
}

impl<C> Simulation<C>
where
    C: NetCommunicator,
{
    pub fn new(
        config: Arc<Config>,
        network: SimNetworkPartition,
        garage: Garage,
        mut population: Population,
        message_broker: NetMessageBroker<C>,
        events: EventsPublisher,
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
            message_broker,
            events,
        }
    }

    pub fn run(&mut self, start_time: u32, end_time: u32) {
        // use fixed start and end times
        let mut now = start_time;
        info!(
            "Starting #{}. Network neighbors: {:?}, Start time {start_time}, End time {end_time}",
            self.message_broker.rank(),
            self.network.neighbors(),
        );

        while now <= end_time {
            if self.message_broker.rank() == 0 && now % 1800 == 0 {
                let _hour = now / 3600;
                let _min = (now % 3600) / 60;
                info!("#{} of Qsim at {_hour}:{_min}", self.message_broker.rank());
            }
            self.wakeup(now);
            self.terminate_teleportation(now);
            self.move_nodes(now);
            self.move_links(now);

            now += 1;
        }

        // maybe this belongs into the controller? Then this would have to be a &mut instead of owned.
        self.events.finish();
    }

    fn wakeup(&mut self, now: u32) {
        let agents = self.activity_q.pop(now);

        for agent in agents {
            let act_type: Id<String> = Id::get(agent.curr_act().act_type);
            self.events.publish_event(
                now,
                &Event::new_act_end(agent.id, agent.curr_act().link_id, act_type.internal()),
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
                    if Simulation::is_local_route(&vehicle, &self.message_broker) {
                        self.teleportation_q.add(vehicle, now);
                    } else {
                        // set the pointer of the route to the last element, so that the current link
                        // is the destination of this leg. Setting this to the last element makes this
                        // logic independent of whether the agent has a Generic-Route with only start
                        // and end link or a full Network-Route, which is often the case for ride modes.
                        vehicle.route_index_to_last();
                        //info!("#{} add vehicle to msg broker.", self.message_broker.rank());
                        self.message_broker.add_veh(vehicle, now);
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
            &Event::new_departure(agent.id, route.start_link(), leg_mode.internal()),
        );

        let veh_id = Id::get(route.veh_id);
        self.garage.unpark_veh(agent, &veh_id)
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
                &Event::new_travelled(agent.id, route.distance, mode.internal()),
            );

            // emmit arrival
            self.events.publish_event(
                now,
                &Event::new_arrival(agent.id, route.end_link(), mode.internal()),
            );

            // advance plan to activity and put agent into activity q.
            agent.advance_plan();

            // emmit act start event
            let act = agent.curr_act();
            let act_type: Id<String> = Id::get(act.act_type);
            self.events.publish_event(
                now,
                &Event::new_act_start(agent.id, act.link_id, act_type.internal()),
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
            let mode = veh_type.net_mode.internal();
            let mut agent = self.garage.park_veh(veh);

            // move to next activity
            agent.advance_plan();
            let act = agent.curr_act();
            self.events
                .publish_event(now, &Event::new_arrival(agent.id, act.link_id, mode));
            let act_type: Id<String> = Id::get(act.act_type);
            self.events.publish_event(
                now,
                &Event::new_act_start(agent.id, act.link_id, act_type.internal()),
            );
            self.activity_q.add(agent, now);
        }
    }

    fn move_links(&mut self, now: u32) {
        let (vehicles, storage_cap) = self.network.move_links(now);

        for veh in vehicles {
            self.message_broker.add_veh(veh, now);
        }

        for cap in storage_cap {
            self.message_broker.add_cap(cap, now);
        }

        let sync_messages = self.message_broker.send_recv(now);

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
        let to = message_broker.rank_for_link(route.end_link());
        message_broker.rank() == to
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::path::PathBuf;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::Arc;
    use std::thread;
    use std::thread::JoinHandle;

    use nohash_hasher::IntMap;
    use tracing::info;

    use crate::simulation::config::Config;
    use crate::simulation::io::xml_events::XmlEventsWriter;
    use crate::simulation::logging;
    use crate::simulation::messaging::events::proto::Event;
    use crate::simulation::messaging::events::{EventsPublisher, EventsSubscriber};
    use crate::simulation::messaging::message_broker::{
        ChannelNetCommunicator, DummyNetCommunicator, NetCommunicator, NetMessageBroker,
    };
    use crate::simulation::network::global_network::Network;
    use crate::simulation::network::sim_network::SimNetworkPartition;
    use crate::simulation::population::population::Population;
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
            DummyNetCommunicator(),
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
        let comms = ChannelNetCommunicator::create_n_2_n(config.num_parts);
        let mut receiver = ReceivingSubscriber::new();

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
    #[ignore]
    fn test_rvr_scenario() {
        let config = Arc::new(
            Config::builder()
                .network_file(String::from(
                    "/Users/janek/Documents/rust_q_sim/input/rvr.network.8.xml.gz",
                ))
                .population_file(String::from(
                    "/Users/janek/Documents/rust_q_sim/input/rvr.1pct.plans.xml.gz",
                ))
                .vehicles_file(String::from(
                    "/Users/janek/Documents/rust_q_sim/input/rvr.vehicles.xml",
                ))
                .output_dir(String::from("/Users/janek/Documents/rust_q_sim/output-wip"))
                .num_parts(8)
                .partition_method(String::from("none"))
                .build(),
        );

        let _guards = logging::init_logging(config.output_dir.as_ref(), 0.to_string());

        let events_path = PathBuf::from(&config.output_dir).join("output.events.xml");

        execute_sim(
            DummyNetCommunicator(),
            Box::new(XmlEventsWriter::new(&events_path)),
            config,
        )
    }

    fn execute_sim<C: NetCommunicator>(
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

        let msg_broker = NetMessageBroker::new(comm, &sim_net, &net);
        let mut events = EventsPublisher::new();
        events.add_subscriber(test_subscriber);

        let mut sim = Simulation::new(config.clone(), sim_net, garage, pop, msg_broker, events);

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
        expected_events: Vec<String>,
    }

    struct ReceivingSubscriber {
        test_subscriber: TestSubscriber,
        channel: (Sender<String>, Receiver<String>),
    }

    struct SendingSubscriber {
        #[allow(dead_code)]
        rank: u32,
        sender: Sender<String>,
    }

    impl EventsSubscriber for SendingSubscriber {
        fn receive_event(&mut self, time: u32, event: &Event) {
            let event_string = XmlEventsWriter::event_2_string(time, event);
            self.sender
                .send(event_string)
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

        fn start_listen(&mut self) {
            while self.test_subscriber.next_index < self.test_subscriber.expected_events.len() {
                let event_string = self
                    .channel
                    .1
                    .recv()
                    .expect("Something went wrong while listening for events");
                self.test_subscriber.receive_event_string(event_string);
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

        fn expected_events() -> Vec<String> {
            let result = vec![
                "<event time=\"32400\" type=\"actend\" person=\"100\" link=\"link1\" actType=\"home\" />\n".to_string(),
                "<event time=\"32400\" type=\"departure\" person=\"100\" link=\"link1\" legMode=\"walk\" />\n".to_string(),
                "<event time=\"32408\" type=\"travelled\" person=\"100\" distance=\"10\" mode=\"walk\" />\n".to_string(),
                "<event time=\"32408\" type=\"arrival\" person=\"100\" link=\"link1\" legMode=\"walk\" />\n".to_string(),
                "<event time=\"32408\" type=\"actstart\" person=\"100\" link=\"link1\" actType=\"car interaction\" />\n".to_string(),
                "<event time=\"32409\" type=\"actend\" person=\"100\" link=\"link1\" actType=\"car interaction\" />\n".to_string(),
                "<event time=\"32409\" type=\"departure\" person=\"100\" link=\"link1\" legMode=\"car\" />\n".to_string(),
                "<event time=\"32409\" type=\"PersonEntersVehicle\" person=\"100\" vehicle=\"100_car\" />\n".to_string(),
                "<event time=\"32419\" type=\"left link\" link=\"link1\" vehicle=\"100_car\" />\n".to_string(),
                "<event time=\"32419\" type=\"entered link\" link=\"link2\" vehicle=\"100_car\" />\n".to_string(),
                "<event time=\"32519\" type=\"left link\" link=\"link2\" vehicle=\"100_car\" />\n".to_string(),
                "<event time=\"32519\" type=\"entered link\" link=\"link3\" vehicle=\"100_car\" />\n".to_string(),
                "<event time=\"32529\" type=\"PersonLeavesVehicle\" person=\"100\" vehicle=\"100_car\" />\n".to_string(),
                "<event time=\"32529\" type=\"arrival\" person=\"100\" link=\"link3\" legMode=\"car\" />\n".to_string(),
                "<event time=\"32529\" type=\"actstart\" person=\"100\" link=\"link3\" actType=\"car interaction\" />\n".to_string(),
                "<event time=\"32530\" type=\"actend\" person=\"100\" link=\"link3\" actType=\"car interaction\" />\n".to_string(),
                "<event time=\"32530\" type=\"departure\" person=\"100\" link=\"link3\" legMode=\"walk\" />\n".to_string(),
                "<event time=\"32546\" type=\"travelled\" person=\"100\" distance=\"20\" mode=\"walk\" />\n".to_string(),
                "<event time=\"32546\" type=\"arrival\" person=\"100\" link=\"link3\" legMode=\"walk\" />\n".to_string(),
                "<event time=\"32546\" type=\"actstart\" person=\"100\" link=\"link3\" actType=\"errands\" />\n".to_string()
            ];
            result
        }
    }

    impl TestSubscriber {
        fn receive_event_string(&mut self, event: String) {
            let expected_value = self.expected_events.get(self.next_index).unwrap();
            self.next_index += 1;
            assert_eq!(expected_value, &event);
        }
    }

    impl EventsSubscriber for TestSubscriber {
        fn receive_event(&mut self, time: u32, event: &Event) {
            self.receive_event_string(XmlEventsWriter::event_2_string(time, event));
        }

        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }
}
