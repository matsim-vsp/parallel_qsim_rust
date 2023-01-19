use crate::mpi::events::proto::event::Type;
use crate::mpi::events::proto::Event;
use crate::mpi::events::proto::TimeStep;
use prost::Message;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, ErrorKind, Read, Seek, Write};
use std::path::Path;

// Include the `events` module, which is generated from events.proto.
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/mpi.events.rs"));
}

struct EventsWriter {
    encoded_events: Vec<u8>,
    curr_time_step: u32,
    writer: BufWriter<File>,
}

impl EventsWriter {
    fn new(path: &Path) -> Self {
        let file = File::create(path).unwrap();
        let writer = BufWriter::new(file);
        EventsWriter {
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
        self.writer
            .flush()
            .expect("Failed to flush buffered writer");
    }
}

impl EventHandler for EventsWriter {
    fn handle_event(&mut self, time: u32, event: &Event) {
        self.update_time_step(time);

        event
            .encode_length_delimited(&mut self.encoded_events)
            .expect("Error encoding event.");
    }

    fn finish(&mut self) {
        self.write_time_step();
    }
}

trait EventHandler {
    fn handle_event(&mut self, time: u32, event: &Event);

    fn finish(&mut self) {}
}

struct EventsManager {
    handlers: Vec<Box<dyn EventHandler>>,
}

/// EventsManager owns event handlers. Handlers are Trait objects, hence they have to be passed in a
/// Box. On handle_event all handler's handle_event methods are called.
impl EventsManager {
    fn new() -> Self {
        EventsManager {
            handlers: Vec::new(),
        }
    }

    fn add_handler(&mut self, handler: Box<dyn EventHandler>) {
        self.handlers.push(handler);
    }

    fn handle_event(&mut self, time: u32, event: &Event) {
        for handler in self.handlers.iter_mut() {
            handler.handle_event(time, event);
        }
    }

    fn finish(&mut self) {
        for handler in self.handlers.iter_mut() {
            handler.finish();
        }
    }
}

struct EventsReader<R: Read + Seek> {
    reader: BufReader<R>,
}

impl<R: Read + Seek> EventsReader<R> {
    fn new(reader: R) -> Self {
        EventsReader {
            reader: BufReader::new(reader),
        }
    }

    pub fn read_next_time_step(&mut self) -> Option<(u32, Vec<Event>)> {
        let delimiter = match self.read_delim() {
            None => return None,
            Some(delim) => delim,
        };
        let time_step = self.read_time_step(delimiter);
        let time = time_step.time;
        let events = self.read_events(time_step);

        Some((time, events))
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

impl EventsReader<File> {
    fn from_file(path: &Path) -> Self {
        let file = File::open(path).unwrap();
        Self::new(file)
    }
}

#[cfg(test)]
mod tests {
    use crate::mpi::events::proto::event::Type::{ActEnd, ActStart, Generic};
    use crate::mpi::events::proto::{
        ActivityEndEvent, ActivityStartEvent, Event, GenericEvent,
    };
    use crate::mpi::events::{match_events, EventsManager, EventsReader, EventsWriter};
    use std::collections::{HashMap, VecDeque};
    use std::fs;
    use std::path::{PathBuf};

    /// This test passes events to the events manager, which holds one events handler. The handler
    /// writes events into an events file as protbuf. Once all events are written events are read
    /// with events reader and then compared.
    #[test]
    fn tinker_test() {
        // create path and corresponding directories
        let path = PathBuf::from("./test_output/events/mpi/protbuf-test.pbf");
        let prefix = path.parent().unwrap();
        fs::create_dir_all(prefix).unwrap();

        // create events manager and add an EventsWriter as only events handler
        let mut manager = EventsManager::new();
        let handler = EventsWriter::new(&path);
        manager.add_handler(Box::new(handler));

        // capture the events we send to the manager, so that we can later assert the written events
        let mut issued_events = VecDeque::new();

        // issue 11 events.
        for i in 0..10 {
            match i {
                0 => {
                    let event = Event {
                        r#type: Some(ActStart(ActivityStartEvent {
                            link: i,
                            act_type: String::from("some-act"),
                            person: 1,
                        })),
                    };
                    manager.handle_event(1, &event);
                    issued_events.push_back(event);
                }
                1 => {
                    let event = Event {
                        r#type: Some(ActEnd(ActivityEndEvent {
                            link: i,
                            act_type: String::from("some-act"),
                            person: 1,
                        })),
                    };
                    manager.handle_event(2, &event);
                    issued_events.push_back(event);
                }
                _ => {
                    let event = Event {
                        r#type: Some(Generic(GenericEvent {
                            r#type: String::from("generic"),
                            attrs: HashMap::from([(String::from("attr1"), String::from("value1"))]),
                        })),
                    };
                    manager.handle_event(3, &event);
                    issued_events.push_back(event);
                }
            }
        }

        let event_other_time_step = Event {
            r#type: Some(Generic(GenericEvent {
                r#type: String::from("another-event-type"),
                attrs: HashMap::from([(
                    String::from("another-attr"),
                    String::from("another-value"),
                )]),
            })),
        };
        manager.handle_event(2, &event_other_time_step);
        issued_events.push_back(event_other_time_step);

        // call finish, so that BufWriter actually writes to file
        manager.finish();

        // create a reader, which reads the written protbuf file
        let mut reader = EventsReader::from_file(&path);

        // iterate over all timesteps the reader can extract from the events file.
        let mut keep_reading = true;
        while keep_reading {
            if let Some((_time, events)) = reader.read_next_time_step() {
                for event in events {
                    let expected_event = issued_events.pop_front().unwrap();
                    match_events(&event, &expected_event);
                }
            } else {
                keep_reading = false;
            }
        }
    }
}

/// Comparing the events is a little tedious becuase they are wrapped in Optional<Type<T>>. This
/// Method unpack all that and then compares actual values.
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
    }
}
