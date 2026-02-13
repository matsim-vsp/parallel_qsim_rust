use derive_builder::Builder;
use rust_qsim::external_services::AdapterHandle;
use rust_qsim::simulation::config::Config;
use rust_qsim::simulation::controller::local_controller::LocalControllerBuilder;
use rust_qsim::simulation::controller::ExternalServices;
use rust_qsim::simulation::events::{EventTrait, EventsManager, OnEventFnBuilder};
use rust_qsim::simulation::io::proto::xml_events::XmlEventsWriter;
use rust_qsim::simulation::scenario::Scenario;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Barrier};
use std::thread;

// If not set here, import gets optimized away.
#[allow(unused_imports)]
use derive_more::Debug;
use rust_qsim::simulation::logging::init_std_out_logging_thread_local;

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
    #[debug(skip)]
    additional_subscribers: HashMap<u32, Vec<Box<OnEventFnBuilder>>>,
    #[builder(default)]
    adapter_handles: Vec<AdapterHandle>,
    #[builder(default = "Arc::new(Barrier::new(1))")]
    global_barrier: Arc<Barrier>,
}

#[allow(dead_code)]
impl TestExecutor<'_> {
    pub fn execute(mut self) {
        let _guard = init_std_out_logging_thread_local();

        // create a test environment
        let (subscribers, receiver) = self.create_test_sub_recv();

        // start the simulation
        self.run(subscribers);

        // start listening for events
        if let Some(mut receiver) = receiver {
            // create another thread for the receiver so that the main thread doesn't block.
            let receiver_handle = thread::Builder::new()
                .name(String::from("test-receiver"))
                .spawn(move || {
                    let _guard = init_std_out_logging_thread_local();
                    receiver.start_listen();
                    drop(_guard)
                })
                .expect("No test receiver thread could be created.");
            receiver_handle
                .join()
                .expect("Failed to join test receiver thread.");
        }

        drop(_guard);
    }

    /// Creates a test subscriber for each partition and a receiving subscriber for the events.
    /// In particular, necessary if simulation is run with multiple threads.
    fn create_test_sub_recv(
        &mut self,
    ) -> (
        HashMap<u32, Vec<Box<OnEventFnBuilder>>>,
        Option<ReceivingSubscriber>,
    ) {
        let mut subscribers: HashMap<u32, Vec<Box<OnEventFnBuilder>>> = HashMap::new();

        let receiver = self
            .expected_events
            .map(ReceivingSubscriber::new_with_events_from_file);

        for c in 0..self.config.partitioning().num_parts {
            let mut subscriber: Vec<Box<OnEventFnBuilder>> =
                if let Some(receiver) = receiver.as_ref() {
                    let subscr = SendingSubscriber::register(c, receiver.channel.0.clone());
                    vec![Box::new(subscr)]
                } else {
                    vec![]
                };

            subscriber.append(
                self.additional_subscribers
                    .get_mut(&c)
                    .unwrap_or(&mut vec![]),
            );
            subscribers.insert(c, subscriber);
        }
        (subscribers, receiver)
    }

    fn run(self, subscribers: HashMap<u32, Vec<Box<OnEventFnBuilder>>>) {
        let scenario = Scenario::load(self.config.clone());

        let controller = LocalControllerBuilder::default()
            .scenario(scenario)
            .events_subscriber_per_partition(subscribers)
            .external_services(self.external_services.clone())
            .global_barrier(self.global_barrier.clone())
            .adapter_handles(self.adapter_handles)
            .build()
            .unwrap();

        controller.run()
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

impl SendingSubscriber {
    fn on_event(&self, event: &dyn EventTrait) {
        let event_string = XmlEventsWriter::event_2_string(event);
        self.sender
            .send(event_string)
            .expect("Failed on sending event message!");
    }

    pub fn register(rank: u32, sender: Sender<String>) -> Box<OnEventFnBuilder> {
        let subscriber = Self { rank, sender };
        Box::new(move |events: &mut EventsManager| {
            events.on_any(move |e| {
                subscriber.on_event(e);
            });
        })
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
        let reader: Box<dyn BufRead> = if rust_qsim::simulation::io::is_url(events_file) {
            #[cfg(feature = "http")]
            {
                let resp = reqwest::blocking::get(events_file).unwrap_or_else(|e| {
                    panic!("Failed to fetch events URL {}: {}", events_file, e)
                });
                let text = resp.text().unwrap_or_else(|e| {
                    panic!("Failed to read response body from {}: {}", events_file, e)
                });
                Box::new(BufReader::new(std::io::Cursor::new(text)))
            }
            #[cfg(not(feature = "http"))]
            {
                panic!("HTTP support is not enabled. Please recompile with the `http` feature enabled.");
            }
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
