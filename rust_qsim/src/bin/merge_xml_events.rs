use std::path::PathBuf;

use clap::Parser;
use rust_qsim::simulation::events::EventsPublisher;
use rust_qsim::simulation::io::proto::xml_events::{XmlEventsReader, XmlEventsWriter};
use rust_qsim::simulation::logging::init_std_out_logging_thread_local;
use tracing::info;

struct StatefulReader {
    reader: XmlEventsReader,
    curr_time_step: u32,
}

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value_t = 1)]
    pub num_parts: u32,
}

fn main() {
    let _g = init_std_out_logging_thread_local();
    let args = InputArgs::parse();
    let mut readers = Vec::new();

    for i in 0..args.num_parts {
        let file_string = format!("{}events.{i}.xml", args.path);
        let file_path = PathBuf::from(file_string);
        let reader = XmlEventsReader::new(&file_path);
        readers.push(StatefulReader {
            reader,
            curr_time_step: 0,
        });
    }

    let mut publisher = EventsPublisher::new();
    XmlEventsWriter::register(PathBuf::from(&args.path).join("events.xml"))(&mut publisher);

    info!("Starting to read events files.");
    while !readers.is_empty() {
        readers.sort_by(|a, b| a.curr_time_step.cmp(&b.curr_time_step));
        let reader = readers.first_mut().unwrap();
        match reader.reader.read_next() {
            None => {
                readers.remove(0);
            }
            Some((time, event)) => {
                if time % 3600 == 0 {
                    info!("Starting time step: {time}");
                }
                publisher.publish_event(event.as_ref());
                reader.curr_time_step = event.time();
            }
        }
    }

    publisher.finish();
}
