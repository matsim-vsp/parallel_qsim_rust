use crate::mpi::events::proto::ActivityStartEvent;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use prost::Message;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

// Include the `events` module, which is generated from events.proto.
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/mpi.events.rs"));
}

struct EventCollector {}

impl EventCollector {
    fn handleActStart(event: ActivityStartEvent) {
        let file = File::create("./test-events.pbf").unwrap();
        let mut writer = BufWriter::new(file);

        event
            .encode_length_delimited(writer)
            .expect("TODO: panic message");
    }
}
