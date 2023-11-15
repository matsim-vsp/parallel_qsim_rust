use std::path::PathBuf;

use clap::Parser;
use tracing::info;

use rust_q_sim::simulation::io::xml_events::{XmlEventsReader, XmlEventsWriter};
use rust_q_sim::simulation::logging;
use rust_q_sim::simulation::messaging::events::EventsPublisher;
use rust_q_sim::simulation::wire_types::events::Event;

struct StatefulReader {
    reader: XmlEventsReader,
    curr_time_step: (u32, Option<Event>),
}

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value_t = 1)]
    pub num_parts: u32,
}

fn main() {
    let args = InputArgs::parse();
    let _guards = logging::init_logging("./", "".to_string());
    let mut readers = Vec::new();

    for i in 0..args.num_parts {
        let file_string = format!("{}events.{i}.xml", args.path);
        let file_path = PathBuf::from(file_string);
        let reader = XmlEventsReader::new(&file_path);
        readers.push(StatefulReader {
            reader,
            curr_time_step: (0, None),
        });
    }

    let mut publisher = EventsPublisher::new();
    publisher.add_subscriber(Box::new(XmlEventsWriter::new(
        &PathBuf::from(&args.path).join("events.xml"),
    )));

    info!("Starting to read events files.");
    while !readers.is_empty() {
        readers.sort_by(|a, b| a.curr_time_step.0.cmp(&b.curr_time_step.0));
        let reader = readers.first_mut().unwrap();
        match reader.reader.read_next() {
            None => {
                readers.remove(0);
            }
            Some((time, event)) => {
                if time % 3600 == 0 {
                    info!("Starting time step: {time}");
                }
                publisher.publish_event(time, &event);
                reader.curr_time_step = (time, Some(event));
            }
        }
    }

    publisher.finish();
}
