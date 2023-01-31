use crate::mpi::events::proto::event::Type;
use crate::mpi::events::proto::Event;
use crate::mpi::events::EventsSubscriber;
use log::info;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub struct XmlEventsWriter {
    writer: BufWriter<File>,
}

impl XmlEventsWriter {
    pub fn new(path: &Path) -> Self {
        info!("Creating file: {path:?}");
        let file = File::create(path).expect("Failed to create File.");
        let mut writer = BufWriter::new(file);
        let header = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<events version=\"1.0\">\n";
        writer
            .write_all(header.as_bytes())
            .expect("Failed to write events file header");
        XmlEventsWriter { writer }
    }

    fn write(&mut self, text: &str) {
        self.writer
            .write_all(text.as_bytes())
            .expect("Error while writing event");
    }
}

impl EventsSubscriber for XmlEventsWriter {
    fn receive_event(&mut self, time: u32, event: &Event) {
        match event.r#type.as_ref().unwrap() {
            Type::Generic(e) => {
                let text = format!(
                    "<event time=\"{time}\" type=\"{}\" attrs is not yet implemented. Sorry/>\n",
                    e.r#type
                );
                self.write(&text);
            }
            Type::ActStart(e) => {
                let text = format!("<event time=\"{time}\" type=\"actstart\" person=\"{}\" link=\"{}\" actType=\"{}\" />\n", e.person, e.link, e.act_type);
                self.write(&text);
            }
            Type::ActEnd(e) => {
                let text = format!("<event time=\"{time}\" type=\"actend\" person=\"{}\" link=\"{}\" actType=\"{}\" />\n", e.person, e.link, e.act_type);
                self.write(&text);
            }
            Type::LinkEnter(e) => {
                let text = format!(
                    "<event time=\"{time}\" type=\"entered link\" link=\"{}\" vehicle=\"{}\" />\n",
                    e.link, e.vehicle
                );
                self.write(&text);
            }
            Type::LinkLeave(e) => {
                let text = format!(
                    "<event time=\"{time}\" type=\"left link\" link=\"{}\" vehicle=\"{}\" />\n",
                    e.link, e.vehicle
                );
                self.write(&text);
            }
            Type::PersonEntersVeh(e) => {
                let text = format!("<event time=\"{time}\" type=\"PersonEntersVehicle\" person=\"{}\" vehicle=\"{}\" />\n", e.person, e.vehicle);
                self.write(&text);
            }
            Type::PersonLeavesVeh(e) => {
                let text = format!("<event time=\"{time}\" type=\"PersonLeavesVehicle\" person=\"{}\" vehicle=\"{}\" />\n", e.person, e.vehicle);
                self.write(&text);
            }
            Type::Departure(e) => {
                let text = format!("<event time=\"{time}\" type=\"departure\" person=\"{}\" link=\"{}\" legMode=\"{}\" />\n", e.person, e.link, e.leg_mode);
                self.write(&text);
            }
            Type::Arrival(e) => {
                let text = format!("<event time=\"{time}\" type=\"arrival\" person=\"{}\" link=\"{}\" legMode=\"{}\" />\n", e.person, e.link, e.leg_mode);
                self.write(&text);
            }
            Type::Travelled(e) => {
                let text = format!("<event time=\"{time}\" type=\"travelled\" person=\"{}\" distance=\"{}\" mode=\"{}\" />\n", e.person, e.distance, e.mode);
                self.write(&text);
            }
        }
    }

    fn finish(&mut self) {
        let closing_tag = "</events>";
        self.write(closing_tag);
        info!("Finishing Events File. Calling flush on Buffered Writer.");
        self.writer.flush().expect("Failed to flush events.");
    }
}
