use derive_builder::Builder;
use nohash_hasher::IntMap;
use rust_qsim::external_services::AdapterHandle;
use rust_qsim::generated::events::Event;
use rust_qsim::simulation::config::Config;
use rust_qsim::simulation::controller::local_controller::LocalControllerBuilder;
use rust_qsim::simulation::controller::ExternalServices;
use rust_qsim::simulation::io::proto::xml_events::XmlEventsWriter;
use rust_qsim::simulation::messaging::events::EventsSubscriber;
use rust_qsim::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use rust_qsim::simulation::scenario::GlobalScenario;
use std::any::Any;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Barrier};
use std::thread;
use std::thread::JoinHandle;

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
// needed because each integration test is build as separate create, thus not all fields are used in each test.
// See https://zerotomastery.io/blog/complete-guide-to-testing-code-in-rust/#Integration-testing
#[allow(dead_code)]
pub struct TestExecutor<'s> {
    config: Arc<Config>,
    #[builder(default)]
    expected_events: Option<&'s str>,
    #[builder(default)]
    external_services: ExternalServices,
    #[builder(default)]
    additional_subscribers: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>>,
    #[builder(default)]
    adapter_handles: Vec<AdapterHandle>,
    #[builder(default = "Arc::new(Barrier::new(1))")]
    global_barrier: Arc<Barrier>,
}

#[allow(dead_code)]
impl TestExecutor<'_> {
    pub fn execute(self) {
        self.execute_config_mutation(|_| {});
    }

    pub fn execute_config_mutation<F>(mut self, config_mutator: F)
    where
        F: Fn(&Arc<Config>),
    {
        config_mutator(&self.config);

        let i = self.config.partitioning().num_parts;
        let handles = if i > 1 {
            self.execute_sim_with_channels()
        } else {
            self.execute_sim()
        };

        rust_qsim::simulation::controller::try_join(handles, self.adapter_handles)
    }

    fn execute_sim_with_channels(&mut self) -> IntMap<u32, JoinHandle<()>> {
        let comms = ChannelSimCommunicator::create_n_2_n(self.config.partitioning().num_parts);

        let mut subscribers: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>> = HashMap::new();

        let receiver = self
            .expected_events
            .map(ReceivingSubscriber::new_with_events_from_file);

        for c in comms {
            if receiver.is_none() {
                continue;
            }

            let subscr = SendingSubscriber {
                rank: c.rank(),
                sender: receiver.as_ref().unwrap().channel.0.clone(),
            };
            let mut subscriber: Vec<Box<dyn EventsSubscriber + Send>> = vec![Box::new(subscr)];
            subscriber.append(
                self.additional_subscribers
                    .get_mut(&c.rank())
                    .unwrap_or(&mut vec![]),
            );
            subscribers.insert(c.rank(), subscriber);
        }

        let scenario = GlobalScenario::build(self.config.clone());

        let controller = LocalControllerBuilder::default()
            .global_scenario(scenario)
            .events_subscriber_per_partition(subscribers)
            .external_services(self.external_services.clone())
            .global_barrier(self.global_barrier.clone())
            .build()
            .unwrap();

        let mut handles = controller.run();

        if let Some(mut receiver) = receiver {
            // create another thread for the receiver, so that the main thread doesn't block.
            let receiver_handle = thread::spawn(move || receiver.start_listen());
            handles.insert(handles.len() as u32, receiver_handle);
        }

        handles
    }

    fn execute_sim(&mut self) -> IntMap<u32, JoinHandle<()>> {
        let mut subscribers = HashMap::new();

        let mut subs: Vec<Box<dyn EventsSubscriber + Send>> =
            if let Some(expected_events) = self.expected_events {
                vec![Box::new(TestSubscriber::new_with_events_from_file(
                    expected_events,
                ))]
            } else {
                vec![]
            };

        subs.append(
            self.additional_subscribers
                .get_mut(&0)
                .unwrap_or(&mut vec![]),
        );

        subscribers.insert(0, subs);

        let scenario = GlobalScenario::build(self.config.clone());

        let controller = LocalControllerBuilder::default()
            .global_scenario(scenario)
            .events_subscriber_per_partition(subscribers)
            .external_services(self.external_services.clone())
            .global_barrier(self.global_barrier.clone())
            .build()
            .unwrap();

        controller.run()
    }
}

pub struct DummySubscriber {}

impl EventsSubscriber for DummySubscriber {
    fn receive_event(&mut self, _: u32, _: &Event) {}

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct TestSubscriber {
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
    fn new_with_events_from_file(events_file: &str) -> Self {
        Self {
            test_subscriber: TestSubscriber::new_with_events_from_file(events_file),
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
    pub fn new_with_events_from_file(events_file: &str) -> Self {
        Self {
            next_index: 0,
            expected_events: Self::expected_events_from_file(events_file),
        }
    }

    /// Load an external file with expected events. Instead of loading proto buf files this has two advantages:
    /// 1. The expected events are in a human-readable format.
    /// 2. The expected events consist of the external ids.
    pub fn expected_events_from_file(events_file: &str) -> Vec<String> {
        let reader: Box<dyn BufRead> = if events_file.starts_with("http://")
            || events_file.starts_with("https://")
        {
            let resp = reqwest::blocking::get(events_file)
                .unwrap_or_else(|e| panic!("Failed to fetch events URL {}: {}", events_file, e));
            let text = resp.text().unwrap_or_else(|e| {
                panic!("Failed to read response body from {}: {}", events_file, e)
            });
            Box::new(BufReader::new(std::io::Cursor::new(text)))
        } else {
            let file = File::open(events_file)
                .unwrap_or_else(|e| panic!("Failed to open events file at {}: {}", events_file, e));
            Box::new(BufReader::new(file))
        };

        reader
            .lines()
            .map(|l| l.unwrap().trim_start().to_string())
            .filter(|s| s.starts_with("<event "))
            .map(|s| s + "\n")
            .collect()
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
