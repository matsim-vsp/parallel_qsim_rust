use std::io::{Read, Seek};
use std::path::PathBuf;

use clap::Parser;
use tracing::info;

use rust_qsim::generated::events::{Event, MyEvent};
use rust_qsim::simulation::events::*;
use rust_qsim::simulation::events::{EventTrait, EventsPublisher, PtTeleportationArrivalEvent};
use rust_qsim::simulation::id;
use rust_qsim::simulation::io::proto::proto_events::ProtoEventsReader;
use rust_qsim::simulation::io::proto::xml_events::XmlEventsWriter;
use rust_qsim::simulation::logging::init_std_out_logging_thread_local;

struct StatefulReader<R: Read + Seek> {
    reader: ProtoEventsReader<R>,
    curr_time_step: (u32, Vec<MyEvent>),
}

impl<R: Read + Seek> StatefulReader<R> {
    pub fn load_next(&mut self) -> Option<()> {
        match self.reader.next() {
            None => None,
            Some(time_step) => {
                self.curr_time_step = time_step;
                Some(())
            }
        }
    }
}

fn main() {
    let _g = init_std_out_logging_thread_local();
    let args = InputArgs::parse();
    info!("Proto2Xml with args: {args:?}");

    info!("Load Id Store");
    id::load_from_file(&PathBuf::from(args.id_store));

    let mut readers = Vec::new();
    info!("Reading from Files: ");
    for i in 0..args.num_parts {
        let file_string = format!("{}events.{i}.binpb", args.path);
        info!("\t {}", file_string);
        let file_path = PathBuf::from(file_string);
        let reader = ProtoEventsReader::from_file(&file_path);
        let wrapper = StatefulReader {
            reader,
            curr_time_step: (0, Vec::new()),
        };
        readers.push(wrapper);
    }
    let output_file_string = format!("{}events.xml.gz", args.path);

    let output_file_path = PathBuf::from(output_file_string);
    let register_xml_writer = XmlEventsWriter::register(output_file_path);

    let mut publisher = EventsPublisher::new();
    register_xml_writer(&mut publisher);

    while !readers.is_empty() {
        readers.sort_by(|a, b| a.curr_time_step.0.cmp(&b.curr_time_step.0));

        // get the reader with the smallest curr time step and process its events
        let reader = readers.first_mut().unwrap();

        process_events(
            reader.curr_time_step.0,
            &reader.curr_time_step.1,
            &mut publisher,
        );
        if reader.load_next().is_none() {
            readers.remove(0);
        };
    }

    info!("Finished reading proto files. Calling finish on XmlWriter");
    publisher.finish();
    info!("Finished writing to xml-file.")
}

#[rustfmt::skip]
fn process_events(time: u32, events: &Vec<MyEvent>, publisher: &mut EventsPublisher) {
    for proto_event in events {
        let type_ = proto_event.attributes["type"].as_string();
        let internal_event: Box<dyn EventTrait> = match type_.as_str() {
            GeneralEvent::TYPE => Box::new(GeneralEvent::from_proto_event(proto_event, time)),
            ActivityStartEvent::TYPE => Box::new(ActivityStartEvent::from_proto_event(proto_event, time)),
            ActivityEndEvent::TYPE => Box::new(ActivityEndEvent::from_proto_event(proto_event, time)),
            LinkEnterEvent::TYPE => Box::new(LinkEnterEvent::from_proto_event(proto_event, time)),
            LinkLeaveEvent::TYPE => Box::new(LinkLeaveEvent::from_proto_event(proto_event, time)),
            PersonEntersVehicleEvent::TYPE => Box::new(PersonEntersVehicleEvent::from_proto_event(proto_event, time)),
            PersonLeavesVehicleEvent::TYPE => Box::new(PersonLeavesVehicleEvent::from_proto_event(proto_event, time)),
            PersonDepartureEvent::TYPE => Box::new(PersonDepartureEvent::from_proto_event(proto_event, time)),
            PersonArrivalEvent::TYPE => Box::new(PersonArrivalEvent::from_proto_event(proto_event, time)),
            TeleportationArrivalEvent::TYPE => Box::new(TeleportationArrivalEvent::from_proto_event(proto_event, time)),
            PtTeleportationArrivalEvent::TYPE => Box::new(PtTeleportationArrivalEvent::from_proto_event(proto_event, time)),
            _ => panic!("Unknown event type: {:?}", type_),
        };
        publisher.publish_event(internal_event.as_ref());
    }
}

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub id_store: String,
    #[arg(long, default_value_t = 1)]
    pub num_parts: u32,
}
