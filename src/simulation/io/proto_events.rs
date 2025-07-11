use std::any::Any;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, ErrorKind, Read, Seek, Write};
use std::path::Path;

use crate::generated::events::{Event, TimeStep};
use crate::simulation::messaging::events::EventsSubscriber;
use prost::Message;

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
}

impl EventsSubscriber for ProtoEventsWriter {
    fn receive_event(&mut self, time: u32, event: &Event) {
        self.update_time_step(time);

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

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct EventsReader<R: Read + Seek> {
    reader: BufReader<R>,
}

impl<R: Read + Seek> EventsReader<R> {
    pub fn new(reader: R) -> Self {
        EventsReader {
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

    fn read_events(&mut self, time_step: TimeStep) -> Vec<Event> {
        let data_len = time_step.data.len() as u64;

        let mut cursor = Cursor::new(time_step.data);
        let mut result = Vec::new();

        while cursor.position() < data_len {
            let event = Event::decode_length_delimited(&mut cursor).expect("Error decoding event");
            result.push(event);
        }

        result
    }
}

impl<R: Read + Seek> Iterator for EventsReader<R> {
    type Item = (u32, Vec<Event>);

    fn next(&mut self) -> Option<Self::Item> {
        let delimiter = self.read_delim()?;
        let time_step = self.read_time_step(delimiter);
        let time = time_step.time;
        let events = self.read_events(time_step);

        Some((time, events))
    }
}

impl EventsReader<File> {
    pub fn from_file(path: &Path) -> Self {
        let file = File::open(path).unwrap_or_else(|_e| panic!("Failed to open File at: {path:?}"));
        Self::new(file)
    }
}

#[cfg(test)]
mod tests {
    use crate::generated::events::event::Type;
    use crate::generated::events::Event;
    use crate::simulation::io::proto_events::{EventsReader, ProtoEventsWriter};
    use crate::simulation::messaging::events::EventsSubscriber;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn write_read_single() {
        let path =
            create_path_with_prefix("./test_output/io/proto_events/write_read_single/events.pbf");
        let mut writer = ProtoEventsWriter::new(&path);
        let event = Event::new_generic(
            "some-event-type",
            HashMap::from([(String::from("attr1"), String::from("value1"))]),
        );
        writer.receive_event(1, &event);
        writer.finish();

        // now read in
        let mut reader = EventsReader::from_file(&path);
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
        let issued_events = vec![
            Event::new_generic(
                "some-event-type",
                HashMap::from([(String::from("attr1"), String::from("value1"))]),
            ),
            Event::new_act_start(1, 1, 1),
            Event::new_act_end(1, 1, 1),
        ];

        for event in &issued_events {
            writer.receive_event(103, event);
        }
        writer.finish();

        // now read in
        let mut reader = EventsReader::from_file(&path);
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
        let issued_events = vec![
            Event::new_generic(
                "some-event-type",
                HashMap::from([(String::from("attr1"), String::from("value1"))]),
            ),
            Event::new_act_start(1, 1, 1),
            Event::new_act_end(1, 1, 1),
        ];

        for time_step in 43..109 {
            for event in &issued_events {
                writer.receive_event(time_step, event);
            }
        }
        writer.finish();

        let reader = EventsReader::from_file(&path);
        let start_time = 43;
        let end_time = 109;
        let mut last_time_step = 42;
        for (time, events) in reader {
            // make sure times are in the correct range and order
            assert!(time >= start_time);
            assert!(time <= end_time);
            assert!(time > last_time_step);
            last_time_step = time;

            assert_eq!(issued_events.len(), events.len());
            for (i, expected_event) in issued_events.iter().enumerate() {
                match_events(expected_event, events.get(i).unwrap());
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

    fn match_events(event: &Event, other: &Event) {
        match event.r#type.as_ref().unwrap() {
            Type::Generic(e) => {
                if let Type::Generic(o) = other.r#type.as_ref().unwrap() {
                    assert_eq!(e.r#type, o.r#type);
                    assert_eq!(e.attrs, o.attrs);
                } else {
                    panic!("wrong type");
                }
            }
            Type::ActStart(e) => {
                if let Type::ActStart(o) = other.r#type.as_ref().unwrap() {
                    assert_eq!(e.person, o.person);
                    assert_eq!(e.act_type, o.act_type);
                    assert_eq!(e.link, o.link);
                } else {
                    panic!("wrong type");
                }
            }
            Type::ActEnd(e) => {
                if let Type::ActEnd(o) = other.r#type.as_ref().unwrap() {
                    assert_eq!(e.person, o.person);
                    assert_eq!(e.act_type, o.act_type);
                    assert_eq!(e.link, o.link);
                } else {
                    panic!("wrong type");
                }
            }
            _ => panic!("Not yet implemented."),
        }
    }
}
