use clap::Parser;
use rust_q_sim::io::proto_events::EventsReader;
use rust_q_sim::io::xml_events::XmlEventsWriter;
use rust_q_sim::mpi::events::proto::Event;
use rust_q_sim::mpi::events::EventsPublisher;
use std::path::PathBuf;

fn main() {
    let args = InputArgs::parse();

    println!("{args:?}");

    let mut readers = Vec::new();
    for i in 0..args.num_parts {
        let file_string = format!("{}.{i}.pbf", args.path);
        println!("{}", file_string);
        let file_path = PathBuf::from(file_string);
        let reader = EventsReader::from_file(&file_path);
        readers.push(reader);
    }
    let output_file_string = format!("{}.xml", args.path);
    let output_file_path = PathBuf::from(output_file_string);
    let mut publisher = EventsPublisher::new();
    publisher.add_subscriber(Box::new(XmlEventsWriter::new(&output_file_path)));

    // assign any value to get over initial test
    let mut readers_with_some = readers.len();

    while readers_with_some > 0 {
        readers_with_some = 0;
        for reader in readers.iter_mut() {
            match reader.next() {
                None => {}
                Some((time, events)) => {
                    readers_with_some += 1;
                    process_events(time, events, &mut publisher);
                }
            }
        }
    }

    publisher.finish();
}

fn process_events(time: u32, events: Vec<Event>, publisher: &mut EventsPublisher) {
    for event in events {
        publisher.publish_event(time, &event);
    }
}

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value_t = 1)]
    pub num_parts: u32,
}
