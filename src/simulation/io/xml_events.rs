use std::any::Any;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use tracing::info;

use crate::simulation::id::Id;
use crate::simulation::messaging::events::proto::event::Type;
use crate::simulation::messaging::events::proto::Event;
use crate::simulation::messaging::events::EventsSubscriber;
use crate::simulation::messaging::messages::proto::{Agent, Vehicle};
use crate::simulation::network::global_network::Link;

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

    pub fn event_2_string(time: u32, event: &Event) -> String {
        match event.r#type.as_ref().unwrap() {
            Type::Generic(e) => {
                format!(
                    "<event time=\"{time}\" type=\"{}\" attrs is not yet implemented. Sorry/>\n",
                    e.r#type
                )
            }
            Type::ActStart(e) => {
                format!("<event time=\"{time}\" type=\"actstart\" person=\"{}\" link=\"{}\" actType=\"{}\" />\n",
                        Id::<Agent>::get(e.person).external(),
                        Id::<Link>::get(e.link).external(),
                        Id::<String>::get(e.act_type).external())
            }
            Type::ActEnd(e) => {
                format!("<event time=\"{time}\" type=\"actend\" person=\"{}\" link=\"{}\" actType=\"{}\" />\n",
                        Id::<Agent>::get(e.person).external(),
                        Id::<Link>::get(e.link).external(),
                        Id::<String>::get(e.act_type).external())
            }
            Type::LinkEnter(e) => {
                format!(
                    "<event time=\"{time}\" type=\"entered link\" link=\"{}\" vehicle=\"{}\" />\n",
                    Id::<Link>::get(e.link).external(),
                    Id::<Vehicle>::get(e.vehicle).external()
                )
            }
            Type::LinkLeave(e) => {
                format!(
                    "<event time=\"{time}\" type=\"left link\" link=\"{}\" vehicle=\"{}\" />\n",
                    Id::<Link>::get(e.link).external(),
                    Id::<Vehicle>::get(e.vehicle).external()
                )
            }
            Type::PersonEntersVeh(e) => {
                format!("<event time=\"{time}\" type=\"PersonEntersVehicle\" person=\"{}\" vehicle=\"{}\" />\n",
                        Id::<Agent>::get(e.person).external(), Id::<Vehicle>::get(e.vehicle).external())
            }
            Type::PersonLeavesVeh(e) => {
                format!("<event time=\"{time}\" type=\"PersonLeavesVehicle\" person=\"{}\" vehicle=\"{}\" />\n",
                        Id::<Agent>::get(e.person).external(), Id::<Vehicle>::get(e.vehicle).external())
            }
            Type::Departure(e) => {
                format!("<event time=\"{time}\" type=\"departure\" person=\"{}\" link=\"{}\" legMode=\"{}\" />\n",
                        Id::<Agent>::get(e.person).external(),
                        Id::<Link>::get(e.link).external(),
                        Id::<String>::get(e.leg_mode).external())
            }
            Type::Arrival(e) => {
                format!("<event time=\"{time}\" type=\"arrival\" person=\"{}\" link=\"{}\" legMode=\"{}\" />\n",
                        Id::<Agent>::get(e.person).external(),
                        Id::<Link>::get(e.link).external(),
                        Id::<String>::get(e.leg_mode).external())
            }
            Type::Travelled(e) => {
                format!("<event time=\"{time}\" type=\"travelled\" person=\"{}\" distance=\"{}\" mode=\"{}\" />\n",
                        Id::<Agent>::get(e.person).external(),
                        e.distance,
                        Id::<String>::get(e.mode).external())
            }
        }
    }

    fn write(&mut self, text: &str) {
        self.writer
            .write_all(text.as_bytes())
            .expect("Error while writing event");
    }
}

impl EventsSubscriber for XmlEventsWriter {
    fn receive_event(&mut self, time: u32, event: &Event) {
        let text = XmlEventsWriter::event_2_string(time, event);
        self.write(&text);
    }

    fn finish(&mut self) {
        let closing_tag = "</events>";
        self.write(closing_tag);
        info!("Finishing Events File. Calling flush on Buffered Writer.");
        self.writer.flush().expect("Failed to flush events.");
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}
