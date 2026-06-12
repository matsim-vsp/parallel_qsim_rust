use crate::generated::events::GenericEvent;
use crate::simulation::events::comparison::EventBatch;
use crate::simulation::events::{EventsManager, comparison};
use crate::simulation::io::proto::proto_events::{ProtoEventsReader, process_events};
use crate::simulation::io::xml::events::{XmlEventsReader, XmlEventsWriter};
use crate::simulation::logging::init_std_out_logging_thread_local;
use crate::simulation::time::SimTime;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Barrier, Mutex};
use std::{fmt, thread};
use tracing::info;

struct StatefulReader<R: Read + Seek> {
    reader: ProtoEventsReader<R>,
    curr_time_step: (SimTime, Vec<GenericEvent>),
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

struct StatefulXmlReader {
    reader: XmlEventsReader,
    curr_time_step: SimTime,
}

/// Reads all proto events from the given folder and publishes them to the given events manager.
/// Assumes that ids are already loaded.
pub fn read_proto_events(
    events: &mut EventsManager,
    folder: &Path,
    prefix: String,
    num_parts: u32,
) {
    let mut readers = Vec::new();
    info!("Reading from Files: ");
    for i in 0..num_parts {
        let path = PathBuf::from(&folder).join(format!("{prefix}.{i}.binpb"));
        info!("\t {}", path.to_str().unwrap());
        let reader = ProtoEventsReader::from_file(&path);
        let wrapper = StatefulReader {
            reader,
            curr_time_step: (SimTime::default(), Vec::new()),
        };
        readers.push(wrapper);
    }

    info!("Starting to read proto files.");
    let mut last_reported_time_step = 0;
    while !readers.is_empty() {
        readers.sort_by(|a, b| a.curr_time_step.0.cmp(&b.curr_time_step.0));

        // get the reader with the smallest curr time step and process its events
        let reader = readers.first_mut().unwrap();

        let secs = reader.curr_time_step.0.as_secs();
        let hour = secs / 3600;
        if hour > last_reported_time_step && secs.is_multiple_of(3600) {
            info!("Reading time step: {:?}h", hour);
            last_reported_time_step = hour;
        }

        process_events(reader.curr_time_step.0, &reader.curr_time_step.1, events);
        if reader.load_next().is_none() {
            readers.remove(0);
        };
    }
    info!("Finished reading proto files.");
    events.finish();
}

/// Reads all XML event files from the given folder and publishes them to the given events manager.
/// Supports both `.xml` and `.xml.gz` files.
/// Assumes that ids are already loaded.
pub fn read_xml_events(
    events_mgr: &mut EventsManager,
    folder: impl AsRef<Path>,
    prefix: String,
    num_parts: u32,
) {
    // initialize readers, one for every file.
    let mut readers = Vec::new();
    info!("Reading from XML Files: ");
    for i in 0..num_parts {
        let path = PathBuf::from(folder.as_ref()).join(format!("{prefix}.{i}.xml"));
        info!("\t {}", path.to_str().unwrap());
        let reader = XmlEventsReader::new(&path);
        let wrapper = StatefulXmlReader {
            reader,
            curr_time_step: SimTime::default(),
        };
        readers.push(wrapper);
    }

    info!("Starting to read XML event files.");
    let mut last_reported_time_step = 0;
    while !readers.is_empty() {
        readers.sort_by(|a, b| a.curr_time_step.cmp(&b.curr_time_step));

        // get the reader with the smallest curr time step and process its event
        let reader = readers.first_mut().unwrap();

        match reader.reader.read_next() {
            None => {
                readers.remove(0);
            }
            Some((time, event)) => {
                let secs = time.as_secs();
                let hour = secs / 3600;
                if hour > last_reported_time_step && secs.is_multiple_of(3600) {
                    info!("Reading time step: {:?}h", hour);
                    last_reported_time_step = hour;
                }
                events_mgr.process_event(event.as_ref());
                reader.curr_time_step = time;
            }
        }
    }
    info!("Finished reading XML event files.");
    events_mgr.finish();
}

/// Reads all proto events from the given folder and writes them to a single XML file (optionally
/// compressed as xml.gz, based on the file extension in the given output path).
/// Assumes that ids are already loaded.
pub fn convert_proto_to_xml_events(
    path_to_proto_files: impl AsRef<Path>,
    num_parts: u32,
    output_file_path: impl AsRef<Path> + 'static + Send + Clone,
) {
    let mut manager = EventsManager::new();

    let register_xml_writer = XmlEventsWriter::register_fn(output_file_path.clone());

    register_xml_writer(&mut manager);

    read_proto_events(
        &mut manager,
        path_to_proto_files.as_ref(),
        String::from("events"),
        num_parts,
    );
    info!(
        "Finished writing to xml file ({}).",
        output_file_path.as_ref().to_str().unwrap()
    );
}

#[derive(Debug, Clone)]
pub enum EventsFileNotEqualError {
    DifferentEventTimes,
    NotChronologicalOrder,
    DifferentNumberOfEvents,
    MissingEvent {
        event: String, // event type and time, of event in file 1 for which no identical event was found in file 2
    },
}

impl fmt::Display for EventsFileNotEqualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventsFileNotEqualError::DifferentEventTimes => write!(f, "Event times differ."),
            EventsFileNotEqualError::NotChronologicalOrder => {
                write!(f, "Events in both files are not in chronological order.")
            }
            EventsFileNotEqualError::DifferentNumberOfEvents => {
                write!(f, "Files have different numbers of events.")
            }
            EventsFileNotEqualError::MissingEvent { event } => write!(
                f,
                "No identical event found in file 2 for an {event} in file 1."
            ),
        }
    }
}

/// Compares two XML event files using parallel reader threads synchronized with a barrier.
/// Two threads read the files independently. When they reach a new timestep, they wait at a barrier.
/// A comparator thread then compares the event batches from both threads. If everything is OK,
/// the threads continue reading.
pub fn compare_xml_event_files(
    file1: impl AsRef<Path>,
    file2: impl AsRef<Path>,
) -> Result<(), EventsFileNotEqualError> {
    let file1_path = file1.as_ref().to_path_buf();
    let file2_path = file2.as_ref().to_path_buf();

    // Initialize shared states between threads
    let batch1 = Arc::new(Mutex::new(EventBatch::new()));
    let batch2 = Arc::new(Mutex::new(EventBatch::new()));
    let comparison_result = Arc::new(Mutex::new(Ok(())));
    let should_stop = Arc::new(AtomicBool::new(false));

    // Barrier for 3 threads: 2 readers + 1 comparator
    let barrier = Arc::new(Barrier::new(3));

    // Spawn reader threads
    let handle1 = comparison::spawn_event_reader(&file1_path, &batch1, &should_stop, &barrier);
    let handle2 = comparison::spawn_event_reader(&file2_path, &batch2, &should_stop, &barrier);

    let comparison_result_cmp = Arc::clone(&comparison_result);

    // Comparator thread
    let handle_cmp = thread::spawn(move || {
        let _guard = init_std_out_logging_thread_local();
        comparison::comparator_thread(
            batch1,
            batch2,
            barrier,
            should_stop,
            comparison_result_cmp,
            file1_path.clone(),
            file2_path.clone(),
        );
        drop(_guard);
    });

    // Wait for all threads
    handle1.join().unwrap();
    handle2.join().unwrap();
    handle_cmp.join().unwrap();

    // Return the comparison result
    let result = comparison_result.lock().unwrap();
    result.clone()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::simulation::events::{EventHandlerRegisterFn, EventTrait};
    use std::rc::Rc;

    /// event handler that writes any event as a string (corresponding to an entry in an XML file)
    /// into a given vector
    struct EventsToVecCollector;

    impl EventsToVecCollector {
        /// on any event, push the event string into the vector
        fn on_any(&self, e: &dyn EventTrait, event_string_collection: Arc<Mutex<Vec<String>>>) {
            event_string_collection
                .lock()
                .unwrap()
                .push(XmlEventsWriter::event_2_string(e));
        }

        /// register the handler to the manager, telling it to call the above function on any event
        pub fn register_fn(
            event_string_collection: Arc<Mutex<Vec<String>>>,
        ) -> Box<EventHandlerRegisterFn> {
            Box::new(move |events: &mut EventsManager| {
                let to_vec_collector = Rc::new(EventsToVecCollector);

                events.on_any(move |e| {
                    to_vec_collector.on_any(e, event_string_collection.clone());
                });
            })
        }
    }

    /// test the read_from_xml function to check that the events read from an XML file are correctly
    /// published to the events manager.
    /// Writes the corresponding xml string of all published events into a vector (using the above
    /// defined EventsToVecCollector event handler), and then comparing the vector with the expected
    /// event strings (corresponding to the events in the read XML file).
    #[test]
    fn test_read_from_xml() {
        let resource_folder = "./tests/resources/events/".to_string();
        let num_parts = 1;

        let mut events_mgr = EventsManager::new();

        // the event strings read from the XML file will be collected in this vector, to be compared
        // with the expected event strings
        let event_string_collection = Arc::new(Mutex::new(Vec::new()));

        // XmlEventsVecCollector is an event handler that writes event strings, like those written
        // into XML, into a given vector.
        let register_xml_event_collector =
            EventsToVecCollector::register_fn(event_string_collection.clone());

        register_xml_event_collector(&mut events_mgr);

        // read the XML events and publish them to the events manager, which will trigger the event
        // handler above and fill the event_string_collection vector
        read_xml_events(
            &mut events_mgr,
            &PathBuf::from(&resource_folder),
            String::from("expected_events"),
            num_parts,
        );

        // assert that the event strings that the events handler handled are all the events inside
        // the read "expected_events.0.xml"
        // (the vector corresponds to the events in tests/resources/events/expected_events.0.xml)
        assert_eq!(
            event_string_collection.lock().unwrap().clone(),
            vec![
                "<event time=\"32400\" type=\"actend\" person=\"100\" link=\"link1\" x=\"5\" y=\"10\" actType=\"home\"/>\n",
                "<event time=\"32400.5\" type=\"departure\" person=\"100\" link=\"link1\" legMode=\"walk\" computationalRoutingMode=\"car\"/>\n",
                "<event time=\"32408\" type=\"travelled\" person=\"100\" distance=\"10\" mode=\"walk\"/>\n",
                "<event time=\"32408\" type=\"arrival\" person=\"100\" link=\"link1\" legMode=\"walk\"/>\n",
                "<event time=\"32409\" type=\"actstart\" person=\"100\" link=\"link1\" x=\"5\" y=\"0\" actType=\"car interaction\"/>\n",
                "<event time=\"32409\" type=\"actend\" person=\"100\" link=\"link1\" x=\"5\" y=\"0\" actType=\"car interaction\"/>\n",
                "<event time=\"32409\" type=\"departure\" person=\"100\" link=\"link1\" legMode=\"car\" computationalRoutingMode=\"car\"/>\n",
                "<event time=\"32409\" type=\"PersonEntersVehicle\" person=\"100\" vehicle=\"100_car\"/>\n",
                "<event time=\"32409.123456789\" type=\"vehicle enters traffic\" person=\"100\" link=\"link1\" vehicle=\"100_car\" networkMode=\"car\" relativePosition=\"1\"/>\n",
                "<event time=\"32410\" type=\"left link\" link=\"link1\" vehicle=\"100_car\"/>\n",
                "<event time=\"32410\" type=\"entered link\" link=\"link2\" vehicle=\"100_car\"/>\n",
                "<event time=\"32511\" type=\"left link\" link=\"link2\" vehicle=\"100_car\"/>\n",
                "<event time=\"32511\" type=\"entered link\" link=\"link3\" vehicle=\"100_car\"/>\n",
                "<event time=\"32521\" type=\"vehicle leaves traffic\" person=\"100\" link=\"link3\" vehicle=\"100_car\" networkMode=\"car\" relativePosition=\"1\"/>\n",
                "<event time=\"32521\" type=\"PersonLeavesVehicle\" person=\"100\" vehicle=\"100_car\"/>\n",
                "<event time=\"32521\" type=\"arrival\" person=\"100\" link=\"link3\" legMode=\"car\"/>\n",
                "<event time=\"32522\" type=\"actstart\" person=\"100\" link=\"link3\" x=\"1100\" y=\"0\" actType=\"car interaction\"/>\n",
                "<event time=\"32522\" type=\"actend\" person=\"100\" link=\"link3\" x=\"1100\" y=\"0\" actType=\"car interaction\"/>\n",
                "<event time=\"32522\" type=\"departure\" person=\"100\" link=\"link3\" legMode=\"walk\" computationalRoutingMode=\"car\"/>\n",
                "<event time=\"32538\" type=\"travelled\" person=\"100\" distance=\"20\" mode=\"walk\"/>\n",
                "<event time=\"32538\" type=\"arrival\" person=\"100\" link=\"link3\" legMode=\"walk\"/>\n",
                "<event time=\"32539\" type=\"actstart\" person=\"100\" link=\"link3\" x=\"1100\" y=\"20\" actType=\"errands\"/>\n"
            ]
        );
    }
}
