use clap::Parser;
use rust_q_sim::io::proto_events::EventsReader;
use rust_q_sim::io::xml_events::XmlEventsWriter;
use rust_q_sim::mpi::events::proto::Event;
use rust_q_sim::mpi::events::EventsPublisher;
use std::io::{Read, Seek};
use std::path::PathBuf;

struct StatefulReader<R: Read + Seek> {
    reader: EventsReader<R>,
    curr_time_step: (u32, Vec<Event>),
}

impl<R: Read + Seek> StatefulReader<R> {
    pub fn load_next(&mut self) -> Option<()> {
        match self.reader.next() {
            None => None,
            Some(time_step) => {
                //println!("load_next {}: {time_step:?}", self.id);
                self.curr_time_step = time_step;
                Some(())
            }
        }
    }
}

fn main() {
    let args = InputArgs::parse();

    println!("Proto2Xml with args: {args:?}");

    let mut readers = Vec::new();
    println!("Reading from Files: ");
    for i in 0..args.num_parts {
        let file_string = format!("{}.{i}.pbf", args.path);
        println!("\t {}", file_string);
        let file_path = PathBuf::from(file_string);
        let reader = EventsReader::from_file(&file_path);
        let wrapper = StatefulReader {
            reader,
            curr_time_step: (0, Vec::new()),
        };
        readers.push(wrapper);
    }
    let output_file_string = format!("{}.xml", args.path);
    let output_file_path = PathBuf::from(output_file_string);
    let mut publisher = EventsPublisher::new();
    publisher.add_subscriber(Box::new(XmlEventsWriter::new(&output_file_path)));

    while !readers.is_empty() {
        readers.sort_by(|a, b| a.curr_time_step.0.cmp(&b.curr_time_step.0));

        // get the reader with the smallest curr time step and process its events
        let reader = readers.first_mut().unwrap();
        /* println!(
            "Process Events of {}: {:?}",
            reader.id, reader.curr_time_step
        );

        */
        process_events(
            reader.curr_time_step.0,
            &reader.curr_time_step.1,
            &mut publisher,
        );
        match reader.load_next() {
            None => {
                readers.remove(0);
            }
            Some(_) => {}
        };
    }

    println!("Finished reading proto files. Calling finish on XmlWriter");
    publisher.finish();
    println!("Finished writing to xml-file.")
}

fn process_events(time: u32, events: &Vec<Event>, publisher: &mut EventsPublisher) {
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
