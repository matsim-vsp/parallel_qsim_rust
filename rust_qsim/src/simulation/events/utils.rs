use crate::generated::events::MyEvent;
use crate::simulation::events::comparision::EventBatch;
use crate::simulation::events::{comparision, EventsManager};
use crate::simulation::io::proto::proto_events::{process_events, ProtoEventsReader};
use crate::simulation::io::proto::xml_events::XmlEventsWriter;
use crate::simulation::logging::init_std_out_logging_thread_local;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Barrier, Mutex};
use std::{fmt, thread};
use tracing::info;

struct StatefulReader<R: Read + Seek> {
    reader: ProtoEventsReader<R>,
    curr_time_step: (u32, Vec<MyEvent>),
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
            curr_time_step: (0, Vec::new()),
        };
        readers.push(wrapper);
    }

    info!("Starting to read proto files.");
    let mut last_reported_time_step = 0;
    while !readers.is_empty() {
        readers.sort_by(|a, b| a.curr_time_step.0.cmp(&b.curr_time_step.0));

        // get the reader with the smallest curr time step and process its events
        let reader = readers.first_mut().unwrap();

        let hour = reader.curr_time_step.0 / 3600;
        if hour > last_reported_time_step && reader.curr_time_step.0 % 3600 == 0 {
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
    let handle1 = comparision::spawn_event_reader(&file1_path, &batch1, &should_stop, &barrier);
    let handle2 = comparision::spawn_event_reader(&file2_path, &batch2, &should_stop, &barrier);

    let comparison_result_cmp = Arc::clone(&comparison_result);

    // Comparator thread
    let handle_cmp = thread::spawn(move || {
        let _guard = init_std_out_logging_thread_local();
        comparision::comparator_thread(
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
