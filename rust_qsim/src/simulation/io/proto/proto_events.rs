use crate::generated::events::{MyEvent, TimeStep};
use crate::generated::general::AttributeValue;
use crate::simulation::events::{
    ActivityEndEvent, ActivityStartEvent, EventTrait, EventsManager, GeneralEvent, LinkEnterEvent,
    LinkLeaveEvent, OnEventFnBuilder, PersonArrivalEvent, PersonDepartureEvent,
    PersonEntersVehicleEvent, PersonLeavesVehicleEvent, PtTeleportationArrivalEvent,
    TeleportationArrivalEvent, VehicleEntersTrafficEvent, VehicleLeavesTrafficEvent,
};
use prost::Message;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, ErrorKind, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

impl From<&ActivityEndEvent> for MyEvent {
    fn from(value: &ActivityEndEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            String::from("person"),
            AttributeValue::from(value.person.external()),
        );
        attributes.insert(
            String::from("link"),
            AttributeValue::from(value.link.external()),
        );
        attributes.insert(
            String::from("act_type"),
            AttributeValue::from(value.act_type.external()),
        );

        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&ActivityStartEvent> for MyEvent {
    fn from(value: &ActivityStartEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "person".to_string(),
            AttributeValue::from(value.person.external()),
        );
        attributes.insert(
            "link".to_string(),
            AttributeValue::from(value.link.external()),
        );
        attributes.insert(
            "act_type".to_string(),
            AttributeValue::from(value.act_type.external()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&LinkEnterEvent> for MyEvent {
    fn from(value: &LinkEnterEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "link".to_string(),
            AttributeValue::from(value.link.external()),
        );
        attributes.insert(
            "vehicle".to_string(),
            AttributeValue::from(value.vehicle.external()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&LinkLeaveEvent> for MyEvent {
    fn from(value: &LinkLeaveEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "link".to_string(),
            AttributeValue::from(value.link.external()),
        );
        attributes.insert(
            "vehicle".to_string(),
            AttributeValue::from(value.vehicle.external()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&PersonEntersVehicleEvent> for MyEvent {
    fn from(value: &PersonEntersVehicleEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "person".to_string(),
            AttributeValue::from(value.person.external()),
        );
        attributes.insert(
            "vehicle".to_string(),
            AttributeValue::from(value.vehicle.external()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&PersonLeavesVehicleEvent> for MyEvent {
    fn from(value: &PersonLeavesVehicleEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "person".to_string(),
            AttributeValue::from(value.person.external()),
        );
        attributes.insert(
            "vehicle".to_string(),
            AttributeValue::from(value.vehicle.external()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&PersonDepartureEvent> for MyEvent {
    fn from(value: &PersonDepartureEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "person".to_string(),
            AttributeValue::from(value.person.external()),
        );
        attributes.insert(
            "link".to_string(),
            AttributeValue::from(value.link.external()),
        );
        attributes.insert(
            "mode".to_string(),
            AttributeValue::from(value.leg_mode.external()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&PersonArrivalEvent> for MyEvent {
    fn from(value: &PersonArrivalEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "person".to_string(),
            AttributeValue::from(value.person.external()),
        );
        attributes.insert(
            "link".to_string(),
            AttributeValue::from(value.link.external()),
        );
        attributes.insert(
            "mode".to_string(),
            AttributeValue::from(value.leg_mode.external()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&TeleportationArrivalEvent> for MyEvent {
    fn from(value: &TeleportationArrivalEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "person".to_string(),
            AttributeValue::from(value.person.external()),
        );
        attributes.insert(
            "mode".to_string(),
            AttributeValue::from(value.mode.external()),
        );
        attributes.insert(
            "distance".to_string(),
            AttributeValue::from(value.distance.to_string()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&PtTeleportationArrivalEvent> for MyEvent {
    fn from(value: &PtTeleportationArrivalEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "person".to_string(),
            AttributeValue::from(value.person.external()),
        );
        attributes.insert(
            "mode".to_string(),
            AttributeValue::from(value.mode.external()),
        );
        attributes.insert(
            "distance".to_string(),
            AttributeValue::from(value.distance.to_string()),
        );
        attributes.insert(
            "route".to_string(),
            AttributeValue::from(value.route.external()),
        );
        attributes.insert(
            "line".to_string(),
            AttributeValue::from(value.line.external()),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&VehicleEntersTrafficEvent> for MyEvent {
    fn from(value: &VehicleEntersTrafficEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "vehicle".to_string(),
            AttributeValue::from(value.vehicle.external()),
        );
        attributes.insert(
            "link".to_string(),
            AttributeValue::from(value.link.external()),
        );
        attributes.insert(
            "driver".to_string(),
            AttributeValue::from(value.driver.external()),
        );
        attributes.insert(
            "mode".to_string(),
            AttributeValue::from(value.mode.external()),
        );
        attributes.insert(
            "relative_position_on_link".to_string(),
            AttributeValue::from(value.relative_position_on_link),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&VehicleLeavesTrafficEvent> for MyEvent {
    fn from(value: &VehicleLeavesTrafficEvent) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "vehicle".to_string(),
            AttributeValue::from(value.vehicle.external()),
        );
        attributes.insert(
            "link".to_string(),
            AttributeValue::from(value.link.external()),
        );
        attributes.insert(
            "driver".to_string(),
            AttributeValue::from(value.driver.external()),
        );
        attributes.insert(
            "mode".to_string(),
            AttributeValue::from(value.mode.external()),
        );
        attributes.insert(
            "relative_position_on_link".to_string(),
            AttributeValue::from(value.relative_position_on_link),
        );
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

impl From<&GeneralEvent> for MyEvent {
    fn from(value: &GeneralEvent) -> Self {
        let mut attributes = HashMap::new();
        for (k, v) in value.attributes.iter() {
            attributes.insert(k.clone(), AttributeValue::from(v.to_string()));
        }
        MyEvent {
            r#type: value.type_().to_string(),
            attributes,
        }
    }
}

pub struct ProtoEventsWriter {
    encoded_events: Vec<u8>,
    curr_time_step: u32,
    writer: BufWriter<File>,
}

impl ProtoEventsWriter {
    pub fn new(path: &Path) -> Self {
        let file = File::create(path).unwrap();
        let writer = BufWriter::new(file);
        ProtoEventsWriter {
            curr_time_step: 0,
            encoded_events: Vec::new(),
            writer,
        }
    }

    fn update_time_step(&mut self, time: u32) {
        if self.curr_time_step != time {
            if !self.encoded_events.is_empty() {
                self.write_time_step();
            }
            self.curr_time_step = time;
        }
    }

    fn write_time_step(&mut self) {
        let mut data: Vec<u8> = Vec::with_capacity(self.encoded_events.len());
        std::mem::swap(&mut data, &mut self.encoded_events);

        let time_step = TimeStep {
            time: self.curr_time_step,
            data,
        };
        let encoded_time_step = time_step.encode_length_delimited_to_vec();

        self.writer
            .write_all(&encoded_time_step)
            .expect("Failed to write all bytes");
    }

    fn convert_to_proto(&mut self, event: &dyn EventTrait) -> MyEvent {
        if let Some(event) = event.as_any().downcast_ref::<GeneralEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<ActivityStartEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<ActivityEndEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<LinkEnterEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<LinkLeaveEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<PersonDepartureEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<PersonArrivalEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<TeleportationArrivalEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<PtTeleportationArrivalEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<VehicleEntersTrafficEvent>() {
            MyEvent::from(event)
        } else if let Some(event) = event.as_any().downcast_ref::<VehicleLeavesTrafficEvent>() {
            MyEvent::from(event)
        } else {
            // TODO use general event here and log warning
            panic!("Unknown event type: {:?}", event);
        }
    }

    fn on_any(&mut self, event: &dyn EventTrait) {
        self.update_time_step(event.time());
        let event = self.convert_to_proto(event);

        event
            .encode_length_delimited(&mut self.encoded_events)
            .expect("Error encoding event.");
    }

    fn finish(&mut self) {
        self.write_time_step();
        self.writer
            .flush()
            .expect("Failed to flush buffered writer.");
    }

    pub fn register(path: PathBuf) -> Box<OnEventFnBuilder> {
        Box::new(move |events: &mut EventsManager| {
            let proto = Rc::new(RefCell::new(ProtoEventsWriter::new(path.as_path())));
            let proto1 = proto.clone();
            let proto2 = proto.clone();

            events.on_any(move |e| {
                proto1.borrow_mut().on_any(e);
            });
            events.on_finish(move || {
                proto2.borrow_mut().finish();
            });
        })
    }
}

pub struct ProtoEventsReader<R: Read + Seek> {
    reader: BufReader<R>,
}

impl<R: Read + Seek> ProtoEventsReader<R> {
    pub fn new(reader: R) -> Self {
        ProtoEventsReader {
            reader: BufReader::new(reader),
        }
    }

    fn read_delim(&mut self) -> Option<usize> {
        // read the delimiter of the message. Prost says delimiter is between 1 and 10 bytes
        // so, read the first 10 bytes of the buffer
        let mut delim_buffer: [u8; 10] = [0; 10];

        // this could crash
        match self.reader.read_exact(&mut delim_buffer) {
            Ok(_) => {} // go on.
            Err(e) => match e.kind() {
                ErrorKind::UnexpectedEof => return None,
                _ => {
                    panic!("Error while reading file: {}", e);
                }
            },
        }
        let delimiter = prost::decode_length_delimiter(delim_buffer.as_slice())
            .expect("error reading delimiter");

        // since the delimiter is a varint figure out how many bytes the delimiter was actually taking
        // up in the buffer. Set the buffers position to the first byte after the delimiter, which
        // should be the start of the TimeStep message
        let delim_encoded_len = prost::encoding::encoded_len_varint(delimiter as u64) as i64;
        let offset = delim_encoded_len - (delim_buffer.len() as i64);
        self.reader
            .seek_relative(offset)
            .expect("Seeking relative failed");

        Some(delimiter)
    }

    fn read_time_step(&mut self, delimiter: usize) -> TimeStep {
        // allocate a buffer with the message length and read into it
        let mut msg_buffer: Vec<u8> = vec![0; delimiter];
        self.reader
            .read_exact(&mut msg_buffer)
            .expect("Error reading msg buffer");

        // then decode it.
        TimeStep::decode(msg_buffer.as_slice()).expect("Could not decode TimeStep message")
    }

    fn read_events(&mut self, time_step: TimeStep) -> Vec<MyEvent> {
        let data_len = time_step.data.len() as u64;

        let mut cursor = Cursor::new(time_step.data);
        let mut result = Vec::new();

        while cursor.position() < data_len {
            let event =
                MyEvent::decode_length_delimited(&mut cursor).expect("Error decoding event");
            result.push(event);
        }

        result
    }
}

impl<R: Read + Seek> Iterator for ProtoEventsReader<R> {
    type Item = (u32, Vec<MyEvent>);

    fn next(&mut self) -> Option<Self::Item> {
        let delimiter = self.read_delim()?;
        let time_step = self.read_time_step(delimiter);
        let time = time_step.time;
        let events = self.read_events(time_step);

        Some((time, events))
    }
}

impl ProtoEventsReader<File> {
    pub fn from_file(path: &Path) -> Self {
        let file = File::open(path).unwrap_or_else(|_e| panic!("Failed to open File at: {path:?}"));
        Self::new(file)
    }
}

#[cfg(test)]
mod tests {
    use crate::generated::events::MyEvent;
    use crate::simulation::events::{
        ActivityEndEvent, ActivityEndEventBuilder, ActivityStartEvent, ActivityStartEventBuilder,
        EventTrait, GeneralEvent, GeneralEventBuilder,
    };
    use crate::simulation::id::Id;
    use crate::simulation::io::proto::proto_events::{ProtoEventsReader, ProtoEventsWriter};
    use crate::simulation::InternalAttributes;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn write_read_single() {
        let path =
            create_path_with_prefix("./test_output/io/proto_events/write_read_single/events.pbf");
        let mut writer = ProtoEventsWriter::new(&path);
        let event: Box<dyn EventTrait> = Box::new(
            GeneralEventBuilder::default()
                .time(1)
                .attributes(InternalAttributes::from(HashMap::from([(
                    String::from("attr1"),
                    String::from("value1"),
                )])))
                .build()
                .unwrap(),
        );
        writer.on_any(event.as_ref());
        writer.finish();

        // now read in
        let mut reader = ProtoEventsReader::from_file(&path);
        let (time, events) = reader.next().expect("Couldn't read timestep.");
        assert_eq!(1, time);
        assert_eq!(1, events.len());
        match_events(&event, events.first().unwrap());
    }

    #[test]
    fn write_read_multiple() {
        let path =
            create_path_with_prefix("./test_output/io/proto_events/write_read_multiple/events.pbf");
        let mut writer = ProtoEventsWriter::new(&path);
        let issued_events: Vec<Box<dyn EventTrait>> = vec![
            Box::new(
                GeneralEventBuilder::default()
                    .time(103)
                    .attributes(InternalAttributes::from(HashMap::from([(
                        String::from("attr1"),
                        String::from("value1"),
                    )])))
                    .build()
                    .unwrap(),
            ),
            Box::new(
                ActivityStartEventBuilder::default()
                    .time(103)
                    .person(Id::create("1"))
                    .link(Id::create("1"))
                    .act_type(Id::create("1"))
                    .build()
                    .unwrap(),
            ),
            Box::new(
                ActivityEndEventBuilder::default()
                    .time(103)
                    .person(Id::create("1"))
                    .link(Id::create("1"))
                    .act_type(Id::create("1"))
                    .build()
                    .unwrap(),
            ),
        ];

        for event in &issued_events {
            writer.on_any(event.as_ref());
        }
        writer.finish();

        // now read in
        let mut reader = ProtoEventsReader::from_file(&path);
        let (time, events) = reader.next().expect("Couldn't read timestep.");
        assert_eq!(103, time);
        assert_eq!(issued_events.len(), events.len());

        for (i, expected_event) in issued_events.iter().enumerate() {
            match_events(expected_event, events.get(i).unwrap());
        }
    }

    #[test]
    fn write_read_multiple_time_steps() {
        let path = create_path_with_prefix(
            "./test_output/io/proto_events/write_read_multiple_time_steps/events.pbf",
        );

        let mut writer = ProtoEventsWriter::new(&path);

        let mut issued_events: Vec<Box<dyn EventTrait>> = Vec::new();

        for time_step in 43..109 {
            let mut v: Vec<Box<dyn EventTrait>> = vec![
                Box::new(
                    GeneralEventBuilder::default()
                        .time(time_step)
                        .attributes(InternalAttributes::from(HashMap::from([(
                            String::from("attr1"),
                            String::from("value1"),
                        )])))
                        .build()
                        .unwrap(),
                ),
                Box::new(
                    ActivityStartEventBuilder::default()
                        .time(time_step)
                        .person(Id::create("1"))
                        .link(Id::create("1"))
                        .act_type(Id::create("1"))
                        .build()
                        .unwrap(),
                ),
                Box::new(
                    ActivityEndEventBuilder::default()
                        .time(time_step)
                        .person(Id::create("1"))
                        .link(Id::create("1"))
                        .act_type(Id::create("1"))
                        .build()
                        .unwrap(),
                ),
            ];
            issued_events.append(&mut v);
        }

        for event in &issued_events {
            writer.on_any(event.as_ref());
        }

        writer.finish();

        let reader = ProtoEventsReader::from_file(&path);
        let start_time = 43;
        let end_time = 109;
        let mut last_time_step = 42;
        for (time, events) in reader {
            // make sure times are in the correct range and order
            assert!(time >= start_time);
            assert!(time <= end_time);
            assert!(time > last_time_step);
            last_time_step = time;

            assert_eq!(3, events.len());
            for (i, event) in events.iter().enumerate() {
                let index = ((time - start_time) * 3) as usize + i;
                match_events(issued_events.get(index).unwrap(), event);
            }
        }
    }

    fn create_path_with_prefix(path: &str) -> PathBuf {
        // create path and corresponding directories
        let path_buf = PathBuf::from(path);
        let prefix = path_buf.parent().unwrap();
        fs::create_dir_all(prefix).unwrap();
        path_buf
    }

    fn match_events(event: &Box<dyn EventTrait>, other: &MyEvent) {
        let type_ = event.type_();
        assert_eq!(type_, other.r#type);

        match type_ {
            GeneralEvent::TYPE => {
                let _typed_event = event.as_any().downcast_ref::<GeneralEvent>().unwrap();
            }
            ActivityStartEvent::TYPE => {
                let typed_event = event.as_any().downcast_ref::<ActivityStartEvent>().unwrap();
                assert_eq!(
                    typed_event.person.external(),
                    other.attributes["person"].as_string()
                );
                assert_eq!(
                    typed_event.link.external(),
                    other.attributes["link"].as_string()
                );
                assert_eq!(
                    typed_event.act_type.external(),
                    other.attributes["act_type"].as_string()
                );
            }
            ActivityEndEvent::TYPE => {
                let typed_event = event.as_any().downcast_ref::<ActivityEndEvent>().unwrap();
                assert_eq!(
                    typed_event.person.external(),
                    other.attributes["person"].as_string()
                );
                assert_eq!(
                    typed_event.link.external(),
                    other.attributes["link"].as_string()
                );
                assert_eq!(
                    typed_event.act_type.external(),
                    other.attributes["act_type"].as_string()
                );
            }
            _ => panic!("wrong type"),
        }
    }
}
