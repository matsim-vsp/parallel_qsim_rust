use crate::generated::events::MyEvent;
use crate::simulation::events::{EventTrait, EventsManager};
use crate::simulation::io::proto::proto_events::{process_events, ProtoEventsReader};
use crate::simulation::io::proto::xml_events::{XmlEventsReader, XmlEventsWriter};
use std::cmp::Ordering;
use std::fmt;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
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
    output_file_path: impl Into<PathBuf> + Clone,
) {
    let mut manager = EventsManager::new();

    let register_xml_writer = XmlEventsWriter::register_fn(Into::into(output_file_path.clone()));

    register_xml_writer(&mut manager);

    read_proto_events(
        &mut manager,
        path_to_proto_files.as_ref(),
        String::from("events"),
        num_parts,
    );
    info!(
        "Finished writing to xml file ({}).",
        Into::into(output_file_path).to_str().unwrap()
    );
}

#[derive(Debug)]
pub enum XmlNotEqualError {
    DifferentEventTimes {
        line: u32,
        time1: u32,
        time2: u32,
    },
    NotChronologicalOrder {
        line: u32,
        current_time: u32,
        last_time: u32,
    },
    DifferentNumberOfEvents {
        file: u32,
        ended_at: u32,
    },
    NoMatchingEvent {
        time: u32,
        event_id: usize,
    },
}

impl fmt::Display for XmlNotEqualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XmlNotEqualError::DifferentEventTimes { line, time1, time2 } => write!(
                f,
                "Events at line {line} have different times: {time1} vs {time2}"
            ),
            XmlNotEqualError::NotChronologicalOrder { line, current_time, last_time } => write!(
                f,
                "Events are not in chronological order: At line {line}, time {current_time} is earlier time {last_time} in the line above"
            ),
            XmlNotEqualError::DifferentNumberOfEvents { file, ended_at } => write!(
                f,
                "File {file} has fewer events than the other file. It ended at line {ended_at}"
            ),
            XmlNotEqualError::NoMatchingEvent { time, event_id } =>
                write!(
                f,
                "No matching event found in file 2 for an event with time {time} in file 1. ¸\nThe \
                event is number {event_id} in the batch of events from file 1 with this time."
            ),
        }
    }
}

/// Compares two XML event files event by event. Panics if any events differ or if the files have
/// different numbers of events.
pub fn compare_xml_event_files(
    file1: impl AsRef<Path>,
    file2: impl AsRef<Path>,
) -> Result<(), XmlNotEqualError> {
    let mut reader1 = XmlEventsReader::new(file1.as_ref());
    let mut reader2 = XmlEventsReader::new(file2.as_ref());

    let mut line_count = 0;
    let mut time_of_last_line: Option<u32> = None;

    // all events with the same time will be compared together, since the order of
    // events with the same time is not guaranteed to be the same in both files.
    // they will be stored here:
    let mut events_with_same_time1: Vec<Box<dyn EventTrait>> = Vec::new();
    let mut events_with_same_time2: Vec<Box<dyn EventTrait>> = Vec::new();

    // go through all events, i.e., lines in the XML files, to compare
    loop {
        let event1 = reader1.read_next();
        let event2 = reader2.read_next();

        match (event1, event2) {
            // if both files indeed have an event in the current line, compare them
            (Some((time1, event_data1)), Some((time2, event_data2))) => {
                // if events in current line have different times, return error, since in this case,
                // either the number of events with the same time differs between the two files, or
                // the events are not sorted increasing in time in one (or both) of the files
                if time1 != time2 {
                    return Err(XmlNotEqualError::DifferentEventTimes {
                        line: line_count,
                        time1,
                        time2,
                    });
                }

                // if the events in the current line have matching time, compare current time to the
                // time of the event in the previous line to decide how to proceed (see below)
                let time_cmp = match time_of_last_line {
                    Some(t_last) => time1.cmp(&t_last),
                    None => Ordering::Equal, // for the first line, set time comparison to Equal,
                                             // since we want to start by filling the batches of
                                             // events with same time
                };

                match time_cmp {
                    // event in current line has earlier time than event in previous line
                    Ordering::Less => {
                        // Not allowed, therefore return error
                        return Err(XmlNotEqualError::NotChronologicalOrder {
                            line: line_count,
                            current_time: time1,
                            last_time: time_of_last_line.unwrap(),
                        });
                    }

                    // event in current line has same time as event in previous line.
                    Ordering::Equal => {
                        // add events of same time to the collection, so they can be compared
                        // together once all such events have been read;
                        events_with_same_time1.push(event_data1);
                        events_with_same_time2.push(event_data2);
                    }

                    // event in current line has later time than event in previous line
                    Ordering::Greater => {
                        // This means that we can compare all the events from the lines before,
                        // which had the same time.
                        // Afterwards, clear the collections with those events, and add the events
                        // of the current line to the collections, to start a new batch of events
                        // to compare

                        // Try to find match for all events in the batch from file 1 in the batch of
                        // events from file 2.
                        // If not possible for some event return the corresponding error
                        match compare_batch_of_events(
                            &events_with_same_time1,
                            &events_with_same_time2,
                        ) {
                            Ok(()) => (),
                            Err(id) => {
                                return Err(XmlNotEqualError::NoMatchingEvent {
                                    time: time_of_last_line.unwrap(),
                                    event_id: id,
                                })
                            }
                        }

                        // clear current batch of events
                        events_with_same_time1.clear();
                        events_with_same_time2.clear();

                        // add events of current line to new batch (will be compared later, when all
                        // events with same time have been read)
                        events_with_same_time1.push(event_data1);
                        events_with_same_time2.push(event_data2);
                    }
                };

                // store time of current line, to compare it with the time of the next line in the
                // next iteration
                time_of_last_line = Some(time1);
            }

            (None, None) => {
                // Once all lines have been read, compare the last batch of events with same time
                match compare_batch_of_events(&events_with_same_time1, &events_with_same_time2) {
                    Ok(()) => (),
                    Err(id) => {
                        return Err(XmlNotEqualError::NoMatchingEvent {
                            time: time_of_last_line.unwrap(),
                            event_id: id,
                        })
                    }
                }

                println!("✓ Successfully compared {} events", line_count);
                break;
            }
            (Some(_), None) => {
                return Err(XmlNotEqualError::DifferentNumberOfEvents {
                    file: 2,
                    ended_at: line_count,
                })
            }
            (None, Some(_)) => {
                return Err(XmlNotEqualError::DifferentNumberOfEvents {
                    file: 1,
                    ended_at: line_count,
                })
            }
        }
        line_count += 1;
    }
    Ok(())
}

fn compare_batch_of_events(
    event_batch_1: &Vec<Box<dyn EventTrait>>,
    event_batch_2: &Vec<Box<dyn EventTrait>>,
) -> Result<(), usize> {
    let mut seen_ids = std::collections::HashSet::new();

    for (id1, event1) in event_batch_1.iter().enumerate() {
        let mut event1_has_match = false;

        // Go through events from file 2 that have same time as event1 from file1
        for (id2, event2) in event_batch_2.iter().enumerate() {
            // skip if event from file 2 has already been matched to (another)
            // event from file 1
            if seen_ids.contains(&id2) {
                continue;
            }

            // check if match is found
            if event1 == event2 {
                event1_has_match = true;
                // Mark id of event2 as seen, so it won't be matched to another event from file1
                seen_ids.insert(id2);
                break;
            }
        }

        // If no match was found for event1 in file2, return an error with the index of the event in
        // the batch
        if !event1_has_match {
            return Err(id1 + 1); // add 1 to id1, since we want to report the event as 1-indexed in the error message
        }
    }

    Ok(())
}
