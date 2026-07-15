use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::rc::Rc;
use std::sync::Mutex;
use tracing::info;
use xml::EventReader;
use xml::attribute::OwnedAttribute;
use xml::reader::XmlEvent;

use crate::simulation::events::{
    ActivityEndEvent, ActivityEndEventBuilder, ActivityStartEvent, ActivityStartEventBuilder,
    EventHandlerRegisterFn, EventTrait, EventsManager, GenericEvent, LinkEnterEvent,
    LinkEnterEventBuilder, LinkLeaveEvent, LinkLeaveEventBuilder, PersonArrivalEvent,
    PersonArrivalEventBuilder, PersonDepartureEvent, PersonDepartureEventBuilder,
    PersonEntersVehicleEvent, PersonEntersVehicleEventBuilder, PersonLeavesVehicleEvent,
    PersonLeavesVehicleEventBuilder, PtTeleportationArrivalEvent, TeleportationArrivalEvent,
    TeleportationArrivalEventBuilder, VehicleEntersTrafficEvent, VehicleEntersTrafficEventBuilder,
    VehicleLeavesTrafficEvent, VehicleLeavesTrafficEventBuilder,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::Coordinate;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::time::SimTime;

pub struct XmlEventsWriter {
    writer: Mutex<Box<dyn Write + Send>>,
}

impl XmlEventsWriter {
    pub fn new(path: impl AsRef<Path>) -> Self {
        info!("Creating file: {:?}", path.as_ref());
        let file = File::create(&path).expect("Failed to create File.");
        let mut writer: Box<dyn Write + Send> = if path.as_ref().extension().unwrap() == "gz" {
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
        if let Some(ev) = e.as_any().downcast_ref::<GenericEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_()
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<ActivityStartEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" x=\"{}\" y=\"{}\" actType=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.coordinate.x,
                ev.coordinate.y,
                ev.act_type
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<ActivityEndEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" x=\"{}\" y=\"{}\" actType=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.coordinate.x,
                ev.coordinate.y,
                ev.act_type
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<LinkEnterEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" link=\"{}\" vehicle=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.link,
                ev.vehicle
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<LinkLeaveEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" link=\"{}\" vehicle=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.link,
                ev.vehicle
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" vehicle=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.vehicle
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" vehicle=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.vehicle
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PersonDepartureEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" legMode=\"{}\" computationalRoutingMode=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.leg_mode,
                ev.routing_mode
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PersonArrivalEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" legMode=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.leg_mode
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<TeleportationArrivalEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" distance=\"{}\" mode=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.distance,
                ev.mode
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<PtTeleportationArrivalEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" distance=\"{}\" mode=\"{}\" line=\"{}\" route=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.distance,
                ev.mode,
                ev.line,
                ev.route
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<VehicleLeavesTrafficEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" vehicle=\"{}\" networkMode=\"{}\" relativePosition=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.vehicle,
                ev.network_mode,
                ev.relative_position
            )
        } else if let Some(ev) = e.as_any().downcast_ref::<VehicleEntersTrafficEvent>() {
            format!(
                "<event time=\"{}\" type=\"{}\" person=\"{}\" link=\"{}\" vehicle=\"{}\" networkMode=\"{}\" relativePosition=\"{}\"/>\n",
                ev.time().format_decimal_seconds(),
                ev.type_(),
                ev.person,
                ev.link,
                ev.vehicle,
                ev.network_mode,
                ev.relative_position
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

    pub fn finish(&self) {
        let closing_tag = "</events>";
        self.write(closing_tag);
        info!("Finishing Events File. Calling flush on Buffered Writer.");
        let mut writer = self.writer.lock().expect("Failed to lock writer");
        writer.flush().expect("Failed to flush events.");
    }

    pub fn register_fn(path: impl AsRef<Path> + Send + 'static) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
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
    parser: EventReader<Box<dyn BufRead>>,
}

impl XmlEventsReader {
    pub fn new(events_file: impl AsRef<Path>) -> Self {
        let file = File::open(events_file.as_ref())
            .unwrap_or_else(|_| panic!("Could not open events file: {:?}", events_file.as_ref()));
        // if events_file is a gz file, read it with a GzDecoder, otherwise read it directly
        let file_is_gz = events_file.as_ref().extension().unwrap() == "gz";
        let buffered_reader: Box<dyn BufRead> = if file_is_gz {
            let gz = flate2::read::GzDecoder::new(file);
            Box::new(BufReader::new(gz))
        } else {
            Box::new(BufReader::new(file))
        };
        let parser = EventReader::new(buffered_reader);
        Self { parser }
    }
    pub fn read_next(&mut self) -> Option<(SimTime, Box<dyn EventTrait>)> {
        loop {
            let result = self.parser.next();
            match result {
                Ok(XmlEvent::StartElement {
                    name, attributes, ..
                }) => {
                    if name.local_name.eq("event") {
                        let time = SimTime::parse_decimal_seconds(
                            value_from_name(&attributes, "time").unwrap(),
                        )
                        .unwrap_or_else(|e| panic!("Could not parse event time: {e}"));
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
        "vehicle enters traffic" => handle_vehicle_enters_traffic(attr),
        "vehicle leaves traffic" => handle_vehicle_leaves_traffic(attr),
        _ => panic!("Unknown event type {ev_type}"),
    }
}

fn handle_vehicle_enters_traffic(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let relative_position: f64 = value_from_name(&attr, "relativePosition")
        .unwrap()
        .parse()
        .unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let link: Id<Link> = Id::create(value_from_name(&attr, "link").unwrap());
    let vehicle: Id<InternalVehicle> = Id::create(value_from_name(&attr, "vehicle").unwrap());
    let network_mode: Id<String> = Id::create(value_from_name(&attr, "networkMode").unwrap());
    Box::new(
        VehicleEntersTrafficEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .vehicle(vehicle)
            .network_mode(network_mode)
            .relative_position(relative_position)
            .build()
            .unwrap(),
    )
}

fn handle_vehicle_leaves_traffic(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let relative_position: f64 = value_from_name(&attr, "relativePosition")
        .unwrap()
        .parse()
        .unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let link: Id<Link> = Id::create(value_from_name(&attr, "link").unwrap());
    let vehicle: Id<InternalVehicle> = Id::create(value_from_name(&attr, "vehicle").unwrap());
    let network_mode: Id<String> = Id::create(value_from_name(&attr, "networkMode").unwrap());
    Box::new(
        VehicleLeavesTrafficEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .vehicle(vehicle)
            .network_mode(network_mode)
            .relative_position(relative_position)
            .build()
            .unwrap(),
    )
}

fn handle_act_end(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let x: f64 = value_from_name(&attr, "x").unwrap().parse().unwrap();
    let y: f64 = value_from_name(&attr, "y").unwrap().parse().unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let link: Id<Link> = Id::create(value_from_name(&attr, "link").unwrap());
    let act_type: Id<String> = Id::create(value_from_name(&attr, "actType").unwrap());
    Box::new(
        ActivityEndEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .act_type(act_type)
            .coordinate(Coordinate::new_2d(x, y))
            .build()
            .unwrap(),
    )
}

fn handle_act_start(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let x: f64 = value_from_name(&attr, "x").unwrap().parse().unwrap();
    let y: f64 = value_from_name(&attr, "y").unwrap().parse().unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let link: Id<Link> = Id::create(value_from_name(&attr, "link").unwrap());
    let act_type: Id<String> = Id::create(value_from_name(&attr, "actType").unwrap());
    Box::new(
        ActivityStartEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .act_type(act_type)
            .coordinate(Coordinate::new_2d(x, y))
            .build()
            .unwrap(),
    )
}

fn handle_departure(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let link: Id<Link> = Id::create(value_from_name(&attr, "link").unwrap());
    let leg_mode: Id<String> = Id::create(value_from_name(&attr, "legMode").unwrap());
    let routing_mode: Id<String> =
        Id::create(value_from_name(&attr, "computationalRoutingMode").unwrap());
    Box::new(
        PersonDepartureEventBuilder::default()
            .time(time)
            .person(person)
            .link(link)
            .leg_mode(leg_mode)
            .routing_mode(routing_mode)
            .build()
            .unwrap(),
    )
}

fn handle_arrival(attr: Vec<OwnedAttribute>) -> Box<dyn EventTrait> {
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let link: Id<Link> = Id::create(value_from_name(&attr, "link").unwrap());
    let leg_mode: Id<String> = Id::create(value_from_name(&attr, "legMode").unwrap());
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
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let distance: f64 = value_from_name(&attr, "distance").unwrap().parse().unwrap();
    let mode: Id<String> = Id::create(value_from_name(&attr, "mode").unwrap());
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
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let vehicle: Id<InternalVehicle> = Id::create(value_from_name(&attr, "vehicle").unwrap());
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
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let person: Id<InternalPerson> = Id::create(value_from_name(&attr, "person").unwrap());
    let vehicle: Id<InternalVehicle> = Id::create(value_from_name(&attr, "vehicle").unwrap());
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
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let link: Id<Link> = Id::create(value_from_name(&attr, "link").unwrap());
    let vehicle: Id<InternalVehicle> = Id::create(value_from_name(&attr, "vehicle").unwrap());
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
    let time = SimTime::parse_decimal_seconds(value_from_name(&attr, "time").unwrap()).unwrap();
    let link: Id<Link> = Id::create(value_from_name(&attr, "link").unwrap());
    let vehicle: Id<InternalVehicle> = Id::create(value_from_name(&attr, "vehicle").unwrap());
    Box::new(
        LinkLeaveEventBuilder::default()
            .time(time)
            .link(link)
            .vehicle(vehicle)
            .build()
            .unwrap(),
    )
}

fn value_from_name<'a>(attr: &'a Vec<OwnedAttribute>, name: &str) -> Option<&'a String> {
    attr.iter()
        .find(|&a| a.name.local_name.eq(name))
        .map(|a| &a.value)
}

#[cfg(test)]
mod tests {
    use super::{XmlEventsReader, XmlEventsWriter};
    use crate::simulation::events::{ActivityStartEvent, ActivityStartEventBuilder, EventTrait};
    use crate::simulation::id::Id;
    use crate::simulation::scenario::Coordinate;
    use crate::simulation::time::SimTime;
    use macros::deterministic_id_test;
    use std::fs;
    use std::path::PathBuf;

    #[deterministic_id_test]
    fn xml_event_round_trip_preserves_nanoseconds() {
        let output_dir = PathBuf::from("./test_output/io/xml_events/nanos_round_trip");
        fs::create_dir_all(&output_dir).unwrap();
        let path = output_dir.join("events.xml");

        let event: Box<dyn EventTrait> = Box::new(
            ActivityStartEventBuilder::default()
                .time(SimTime::from_nanos(42_123_456_789))
                .person(Id::create("person-1"))
                .link(Id::create("link-1"))
                .act_type(Id::create("home"))
                .coordinate(Coordinate::new_2d(1.0, 2.0))
                .build()
                .unwrap(),
        );

        let writer = XmlEventsWriter::new(&path);
        writer.on_any(event.as_ref());
        writer.finish();

        let mut reader = XmlEventsReader::new(&path);
        let (time, parsed_event) = reader.read_next().unwrap();

        assert_eq!(SimTime::from_nanos(42_123_456_789), time);

        let parsed_event = parsed_event
            .as_any()
            .downcast_ref::<ActivityStartEvent>()
            .unwrap();
        assert_eq!(Id::create("person-1"), parsed_event.person);
        assert_eq!(Id::create("link-1"), parsed_event.link);
        assert_eq!(Id::create("home"), parsed_event.act_type);
        assert_eq!(Coordinate::new_2d(1.0, 2.0), parsed_event.coordinate);
    }
}
