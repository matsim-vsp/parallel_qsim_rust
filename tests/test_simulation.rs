use std::any::Any;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use rust_q_sim::generated::events::Event;
use rust_q_sim::simulation::config::{CommandLineArgs, Config};
use rust_q_sim::simulation::io::proto::xml_events::XmlEventsWriter;
use rust_q_sim::simulation::messaging::events::EventsSubscriber;
use rust_q_sim::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use rust_q_sim::simulation::messaging::sim_communication::SimCommunicator;

pub fn execute_sim_with_channels(config_args: CommandLineArgs, expected_events: &str) {
    let config = Config::from_file(&config_args);
    let comms = ChannelSimCommunicator::create_n_2_n(config.partitioning().num_parts);
    let mut receiver = ReceivingSubscriber::new_with_events_from_file(expected_events);

    let mut subscribers: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>> = HashMap::new();

    for c in comms {
        let subscr = SendingSubscriber {
            rank: c.rank(),
            sender: receiver.channel.0.clone(),
        };
        subscribers.insert(c.rank(), vec![Box::new(subscr)]);
    }

    let mut handles = rust_q_sim::simulation::controller::local_controller::run_channel(
        Config::from_file(&config_args),
        config_args,
        subscribers,
        Default::default(),
    );

    // create another thread for the receiver, so that the main thread doesn't block.
    let receiver_handle = thread::spawn(move || receiver.start_listen());
    handles.insert(handles.len() as u32, receiver_handle);

    rust_q_sim::simulation::controller::try_join(handles, Default::default())
}

pub fn execute_sim(
    subscriber: Vec<Box<dyn EventsSubscriber + Send>>,
    config_args: CommandLineArgs,
) {
    let mut subscribers = HashMap::new();
    subscribers.insert(0, subscriber);

    rust_q_sim::simulation::controller::local_controller::run_channel(
        Config::from_file(&config_args),
        config_args,
        subscribers,
        Default::default(),
    );
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
    /// 1. The expected events are in a human readable format.
    /// 2. The expected events consist of the external ids.
    fn expected_events_from_file(events_file: &str) -> Vec<String> {
        let file = File::open(events_file).expect("Failed to open events file.");
        let reader = BufReader::new(file);

        // Prepare the expected events. Since the file is an xml events file, we do not want to compare other lines than the
        // event lines. Also, we need to append \n to each line since the reader strips it.
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
