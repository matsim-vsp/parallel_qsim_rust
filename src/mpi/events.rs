use crate::mpi::events::proto::{ActivityEndEvent, ActivityStartEvent, GenericEvent, TimeStep};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use mpi::time;
use prost::Message;
use std::alloc::handle_alloc_error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

// Include the `events` module, which is generated from events.proto.
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/mpi.events.rs"));
}

struct EventCollector {
    encoded_events: Vec<u8>,
    curr_time_step: u32,
    writer: BufWriter<File>,
}

impl EventCollector {
    fn new(path: &Path) -> Self {
        let file = File::create(path).unwrap();
        let writer = BufWriter::new(file);
        EventCollector {
            curr_time_step: 0,
            encoded_events: Vec::new(),
            writer,
        }
    }

    fn write_time_step(&mut self) {
        println!("handler write time step");
        let mut data: Vec<u8> = Vec::with_capacity(self.encoded_events.len());
        std::mem::swap(&mut data, &mut self.encoded_events);

        let time_step = TimeStep {
            time: self.curr_time_step,
            data,
        };
        let encoded_time_step = time_step.encode_length_delimited_to_vec();

        println!("Handler writing encoded time step");
        self.writer.write(&encoded_time_step).unwrap();
    }
}

impl EventHandler for EventCollector {
    fn handle_act_start(&mut self, time: u32, event: &ActivityStartEvent) {
        if time != self.curr_time_step {
            self.write_time_step();
        }

        event
            .encode_length_delimited(&mut self.encoded_events)
            .unwrap();
        println!("{:?}", self.encoded_events);
    }

    fn finish(&mut self) {
        println!("Handler finish");
        self.write_time_step();
    }
}

trait EventHandler {
    // provide default implementations which don't do anything
    fn handle_act_start(&mut self, time: u32, event: &ActivityStartEvent) {}
    fn handle_act_end(&mut self, time: u32, event: &ActivityEndEvent) {}

    fn handle_generic(&mut self, time: u32, event: &GenericEvent) {}

    fn finish(&mut self) {}
}

struct EventsManager {
    handlers: Vec<Box<dyn EventHandler>>,
}

impl EventsManager {
    fn new() -> Self {
        EventsManager {
            handlers: Vec::new(),
        }
    }

    fn add_handler(&mut self, handler: Box<dyn EventHandler>) {
        self.handlers.push(handler);
    }

    fn act_start_event(&mut self, time: u32, person: u64, link: u64, act_type: &str) {
        let event = ActivityStartEvent {
            act_type: String::from(act_type),
            link,
            person,
        };
        for handler in self.handlers.iter_mut() {
            handler.handle_act_start(time, &event);
        }
    }

    fn finish(&mut self) {
        for handler in self.handlers.iter_mut() {
            handler.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::mpi::events::proto::TimeStep;
    use crate::mpi::events::{EventCollector, EventsManager};
    use prost::Message;
    use std::fs::File;
    use std::io::{BufReader, Cursor};
    use std::path::{Path, PathBuf};

    #[test]
    fn tinker_test() {
        // wire up
        let path = PathBuf::from("./test_output/protbuf-test.pbf");
        let mut manager = EventsManager::new();
        let handler = EventCollector::new(&path);
        manager.add_handler(Box::new(handler));

        println!("First event");
        manager.act_start_event(1, 1, 1, "some");
        println!("Second event");
        manager.act_start_event(1, 1, 2, "another");

        read_file(&path);
    }

    fn read_file(path: &Path) {
        println!("Start reading file");
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        //reader.
        //let cursor = Cursor::new(reader);

        let time_step = TimeStep::decode_length_delimited(reader).unwrap();

        println!("{:?}", time_step);
    }
}
