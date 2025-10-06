use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;
use tracing::info;
use xml::attribute::OwnedAttribute;
use xml::reader::XmlEvent;
use xml::EventReader;

use crate::simulation::events::{
    ActivityEndEvent, ActivityEndEventBuilder, ActivityStartEvent, ActivityStartEventBuilder,
    EventTrait, EventsPublisher, GeneralEvent, LinkEnterEvent, LinkEnterEventBuilder,
    LinkLeaveEvent, LinkLeaveEventBuilder, OnEventFnBuilder, PersonArrivalEvent,
    PersonArrivalEventBuilder, PersonDepartureEvent, PersonDepartureEventBuilder,
    PersonEntersVehicleEvent, PersonEntersVehicleEventBuilder, PersonLeavesVehicleEvent,
    PersonLeavesVehicleEventBuilder, PtTeleportationArrivalEvent, TeleportationArrivalEvent,
    TeleportationArrivalEventBuilder,
};
use crate::simulation::id::Id;
use crate::simulation::network::Link;
use crate::simulation::population::InternalPerson;
use crate::simulation::vehicles::InternalVehicle;

pub struct XmlEventsWriter {
    writer: Mutex<Box<dyn Write + Send>>,
}

impl XmlEventsWriter {
    pub fn new(path: PathBuf) -> Self {
        info!("Creating file: {path:?}");
        let file = File::create(&path).expect("Failed to create File.");
        let mut writer: Box<dyn Write + Send> = if path.extension().unwrap() == "gz" {
            Box::new(GzEncoder::new(file, Compression::fast()))
        } else {
            Box::new(BufWriter::new(file))
        };
        let header = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<events version=\"1.0\">\n";
        writer
            .write_all(header.as_bytes())
            .expect("Failed to write events file header");
        XmlEventsWriter {
            writer: Mutex::new(writer),
        }
    }

    pub fn event_2_string(e: &dyn EventTrait) -> String {
        if let Some(ev) = e.as_any().downcast_ref::<GeneralEvent>() {
            format!("<event time=\"{}\" type=\"{}\"/>\n", ev.time(), ev.type_())
        } else if let Some(ev) = e.as_any().downcast_ref::<ActivityStartEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" actType=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.act_type
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<ActivityEndEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" actType=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.act_type
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<LinkEnterEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" link=\"{}\" vehicle=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.link,
                ev.vehicle
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<LinkLeaveEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" link=\"{}\" vehicle=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.link,
                ev.vehicle
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" vehicle=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.person,
                ev.vehicle
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" vehicle=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.person,
                ev.vehicle
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PersonDepartureEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" legMode=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.leg_mode
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PersonArrivalEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" legMode=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.leg_mode
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<TeleportationArrivalEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" distance=\"{}\" mode=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.person,
                ev.distance,
                ev.mode
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PtTeleportationArrivalEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" mode=\"{}\" distance=\"{}\" route=\"{}\" line=\"{}\"/>\n",
                ev.time(),
                ev.type_(),
                ev.person,
                ev.mode,
                ev.distance,
                ev.route,
                ev.line
            )
        } else {
            panic!("Unknown event type");
        }
    }

    pub fn on_any(&self, e: &dyn EventTrait) {
        self.write(&Self::event_2_string(e));
    }

    fn write(&self, text: &str) {
        let mut writer = self.writer.lock().expect("Failed to lock writer");
        writer
            .write_all(text.as_bytes())
            .expect("Error while writing event");
    }

    fn finish(&self) {
        let closing_tag = "</events>";
        self.write(closing_tag);
        info!("Finishing Events File. Calling flush on Buffered Writer.");
        let mut writer = self.writer.lock().expect("Failed to lock writer");
        writer.flush().expect("Failed to flush events.");
    }

    pub fn register(path: PathBuf) -> Box<OnEventFnBuilder> {
        Box::new(move |events: &mut EventsPublisher| {
            let xml = Rc::new(XmlEventsWriter::new(path));
            let xml1 = xml.clone();
            let xml2 = xml.clone();

            events.on_any(move |e| {
                xml1.on_any(e);
            });
            events.on_finish(move || {
                xml2.finish();
            })
        })
    }
}

pub struct XmlEventsReader {
    parser: EventReader<BufReader<File>>,
}

impl XmlEventsReader {
    pub fn new(events_file: &Path) -> Self {
        let file = File::open(events_file)
            .unwrap_or_else(|_| panic!("Could not open events file: {:?}", events_file));
        let buffered_reader = BufReader::new(file);
        let parser = EventReader::new(buffered_reader);
        Self { parser }
    }
    pub fn read_next(&mut self) -> Option<(u32, Box<dyn EventTrait>)> {
        loop {
            let result = self.parser.next();
            match result {
                Ok(XmlEvent::StartElement {
                    name, attributes, ..
                }) => {
                    if name.local_name.eq("event") {
                        let time: u32 = attributes.first().unwrap().value.parse().unwrap();
                        let event = handle(attributes);
                        return Some((time, event));
                    }
                }
                Ok(XmlEvent::EndDocument) => return None,
                Err(_) => return None,
                _ => {
                    continue;
                }
            }
        }
    }
}

fn handle(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let ev_type = &attr.get(1).unwrap().value;
    match ev_type.as_str() {
        "actend" => handle_act_end(attr),
        "departure" => handle_departure(attr),
        "travelled" => travelled(attr),
        "arrival" => handle_arrival(attr),
        "actstart" => handle_act_start(attr),
        "PersonEntersVehicle" => handle_person_enters_veh(attr),
        "PersonLeavesVehicle" => handle_person_leaves_veh(attr),
        "entered link" => handle_link_enter(attr),
        "left link" => handle_link_leave(attr),
        _ => panic!("Unknown event type {ev_type}"),
    }
}

fn handle_act_end(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let person: Id<InternalPerson> = Id::create(&attr.get(2).unwrap().value);
    let link: Id<Link> = Id::create(&attr.get(3).unwrap().value);
    let act_type: Id<String> = Id::create(&attr.get(4).unwrap().value);
    Box::new(
        ActivityEndEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .act_type(act_type)
            .build()
            .unwrap(),
    )
}

fn handle_act_start(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let person: Id<InternalPerson> = Id::create(&attr.get(2).unwrap().value);
    let link: Id<Link> = Id::create(&attr.get(3).unwrap().value);
    let act_type: Id<String> = Id::create(&attr.get(4).unwrap().value);
    Box::new(
        ActivityStartEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .act_type(act_type)
            .build()
            .unwrap(),
    )
}

fn handle_departure(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let person: Id<InternalPerson> = Id::create(&attr.get(2).unwrap().value);
    let link: Id<Link> = Id::create(&attr.get(3).unwrap().value);
    let leg_mode: Id<String> = Id::create(&attr.get(4).unwrap().value);
    Box::new(
        PersonDepartureEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .leg_mode(leg_mode)
            .build()
            .unwrap(),
    )
}

fn handle_arrival(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let person: Id<InternalPerson> = Id::create(&attr.get(2).unwrap().value);
    let link: Id<Link> = Id::create(&attr.get(3).unwrap().value);
    let leg_mode: Id<String> = Id::create(&attr.get(4).unwrap().value);
    Box::new(
        PersonArrivalEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .leg_mode(leg_mode)
            .build()
            .unwrap(),
    )
}

fn travelled(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let person: Id<InternalPerson> = Id::create(&attr.get(2).unwrap().value);
    let distance: f64 = attr.get(3).unwrap().value.parse().unwrap();
    let mode: Id<String> = Id::create(&attr.get(4).unwrap().value);
    Box::new(
        TeleportationArrivalEventBuilder::default()
            .time(time)
            .person(person)
            .mode(mode)
            .distance(distance)
            .build()
            .unwrap(),
    )
}

fn handle_person_enters_veh(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let person: Id<InternalPerson> = Id::create(&attr.get(2).unwrap().value);
    let vehicle: Id<InternalVehicle> = Id::create(&attr.get(3).unwrap().value);
    Box::new(
        PersonEntersVehicleEventBuilder::default()
            .time(time)
            .person(person)
            .vehicle(vehicle)
            .build()
            .unwrap(),
    )
}

fn handle_person_leaves_veh(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let person: Id<InternalPerson> = Id::create(&attr.get(2).unwrap().value);
    let vehicle: Id<InternalVehicle> = Id::create(&attr.get(3).unwrap().value);
    Box::new(
        PersonLeavesVehicleEventBuilder::default()
            .time(time)
            .person(person)
            .vehicle(vehicle)
            .build()
            .unwrap(),
    )
}

fn handle_link_enter(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let link: Id<Link> = Id::create(&attr.get(2).unwrap().value);
    let vehicle: Id<InternalVehicle> = Id::create(&attr.get(3).unwrap().value);
    Box::new(
        LinkEnterEventBuilder::default()
            .time(time)
            .link(link)
            .vehicle(vehicle)
            .build()
            .unwrap(),
    )
}

fn handle_link_leave(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time: u32 = attr.first().unwrap().value.parse().unwrap();
    let link: Id<Link> = Id::create(&attr.get(2).unwrap().value);
    let vehicle: Id<InternalVehicle> = Id::create(&attr.get(3).unwrap().value);
    Box::new(
        LinkLeaveEventBuilder::default()
            .time(time)
            .link(link)
            .vehicle(vehicle)
            .build()
            .unwrap(),
    )
}
