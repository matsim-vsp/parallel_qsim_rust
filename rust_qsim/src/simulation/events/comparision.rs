use crate::simulation::events::utils::EventsFileNotEqualError;
use crate::simulation::events::EventTrait;
use crate::simulation::io::proto::xml_events::XmlEventsReader;
use crate::simulation::logging::init_std_out_logging_thread_local;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::thread::JoinHandle;
use tracing::error;

pub fn spawn_event_reader(
    file_path: &Path,
    batch: &Arc<Mutex<EventBatch>>,
    should_stop: &Arc<AtomicBool>,
    barrier: &Arc<Barrier>,
) -> JoinHandle<()> {
    let batch_clone = Arc::clone(batch);
    let barrier_clone = Arc::clone(barrier);
    let should_stop_clone = Arc::clone(should_stop);
    let file1_path_clone = file_path.to_path_buf();
    thread::spawn(move || {
        let _guard = init_std_out_logging_thread_local();
        read_file_with_barrier(
            file1_path_clone,
            batch_clone,
            barrier_clone,
            should_stop_clone,
        );
        drop(_guard)
    })
}

/// Reads an XML event file and synchronizes via a barrier
fn read_file_with_barrier(
    file_path: PathBuf,
    batch: Arc<Mutex<EventBatch>>,
    barrier: Arc<Barrier>,
    should_stop: Arc<AtomicBool>,
) {
    let mut reader = XmlEventsReader::new(&file_path);
    let mut current_time: Option<u32> = None;
    let mut current_events: Vec<Box<dyn EventTrait>> = Vec::new();

    loop {
        // using Relaxed ordering for should_stop for synchronization of this memory only.
        if should_stop.load(AtomicOrdering::Relaxed) {
            break;
        }

        match reader.read_next() {
            Some((time, event_data)) => {
                // If current_time is not None, we have already read events from the file
                if let Some(curr_time) = current_time {
                    // In that case, check if the time of the current event is different from the
                    // current_time.
                    if time != curr_time {
                        // If so, we need to publish the batch of events with the current_time and
                        // start a new batch for the new time.
                        update_batch(&batch, &mut current_time, &mut current_events, false);

                        // Phase 1: publish batch
                        // i.e., wait for the other reader to also be ready with its batch. Only
                        // then the comparator starts comparing the two
                        barrier.wait();

                        // Phase 2: wait for comparator to consume/decide
                        // i.e., do nothing while the comparator compares the batches
                        barrier.wait();

                        if should_stop.load(AtomicOrdering::Relaxed) {
                            break;
                        }

                        current_time = Some(time);
                        current_events.push(event_data);
                    } else {
                        // If time of current event is current_time, we extend the current batch
                        // with the new event.
                        current_events.push(event_data);
                    }
                } else {
                    // If current_time is None, this is the first event we read from the file, so we
                    // just start a new batch with its time and event data.
                    current_time = Some(time);
                    current_events.push(event_data);
                }
            }
            // If reader.read_next() returns None, we have reached the end of the file.
            None => {
                // In that case, we need to publish the last batch of events (if there are any) and
                // then break the loop
                update_batch(&batch, &mut current_time, &mut current_events, true);

                // Final publish/consume cycle
                barrier.wait();
                barrier.wait();
                break;
            }
        }
    }
}

/// Structure to hold a batch of events with the same timestamp from one file
#[derive(Debug)]
pub(super) struct EventBatch {
    time: Option<u32>,
    events: Vec<Box<dyn EventTrait>>,
    finished: bool, // indicates if the reader has reached EOF
}

impl EventBatch {
    pub(super) fn new() -> Self {
        Self {
            time: None,
            events: Vec::new(),
            finished: false,
        }
    }
}

/// Updates a given batch with the current time and events.
fn update_batch(
    batch: &Arc<Mutex<EventBatch>>,
    current_time: &mut Option<u32>,
    current_events: &mut Vec<Box<dyn EventTrait>>,
    finished: bool,
) {
    let mut batch_lock = batch.lock().unwrap();
    batch_lock.time = *current_time;
    batch_lock.events = std::mem::take(current_events);
    batch_lock.finished = finished;
}

/// Comparator thread: compares the event batches from both reader threads. Breaks loop when
/// returning errors or when both readers have reached EOF.
/// Order of comparison in the loop:
/// 1) If both readers have reached EOF, do the final checks (time matches between batches, events in batches match) and break the loop
/// 2) If only one reader has reached EOF, log error (DifferentNumberOfEvents) and break the loop
/// 3) Compare times of batches: if times differ, log error (DifferentEventTimes);
///    if time is smaller than last time, log error (NotChronologicalOrder);
///    else compare events in the batches, log error (MissingEvent) if they don't match.
pub fn comparator_thread(
    batch1: Arc<Mutex<EventBatch>>,
    batch2: Arc<Mutex<EventBatch>>,
    barrier: Arc<Barrier>,
    should_stop: Arc<AtomicBool>,
    comparison_result: Arc<Mutex<Result<(), EventsFileNotEqualError>>>,
    file1: PathBuf,
    file2: PathBuf,
) {
    let mut event_count = 0;
    let mut last_time: Option<u32> = None;

    loop {
        // start by waiting for both readers to publish their batches
        barrier.wait();

        // then start comparison
        let (time1, events1, finished1) = extract_events(batch1.clone());
        let (time2, events2, finished2) = extract_events(batch2.clone());

        // if both readers are finished (have reached EOF), do the final checks and break the loop
        if finished1 && finished2 {
            // if there are events in the last batch
            if let (Some(t1), Some(t2)) = (time1, time2) {
                // check if their times match between the files
                if t1 != t2 {
                    handle_different_event_times(
                        &should_stop,
                        &comparison_result,
                        &mut event_count,
                        &events1,
                        &events2,
                    );
                }
                // matching times, but comparing the batches yielded a difference
                else if let Err(id) = compare_batch_of_events(&events1, &events2) {
                    handle_missing_event(&should_stop, &comparison_result, &events1, id);
                }
                // otherwise, all good
            }

            // wake up the reader threads one last time to let them break their loops
            // and then break this loop
            barrier.wait();
            break;
        }

        // only one reader finished (reached EOF)
        if finished1 != finished2 {
            // log error, depending on which file finished
            match finished1 {
                true => error!(
                    "Reached end of file for {}. It has only {} events. There are still events in file {}.",
                    file1.to_str().unwrap(),
                    event_count,
                    file2.to_str().unwrap()
                ),
                false => error!(
                    "Reached end of file for {}. It has only {} events. There are still events in file {}.",
                    file2.to_str().unwrap(),
                    event_count,
                    file1.to_str().unwrap()
                ),
            }

            let mut result = comparison_result.lock().unwrap();
            *result = Err(EventsFileNotEqualError::DifferentNumberOfEvents);
            should_stop.store(true, AtomicOrdering::Relaxed);

            // wake up the reader threads one last time to let them break their loops
            // and then break this loop
            barrier.wait();
            break;
        }

        // no reader finished. proceed to compare current batches of events.

        // check time of both event batches
        match (time1, time2) {
            (Some(t1), Some(t2)) => {
                // if times differ between the batches, then something is wrong
                // (missing events or wrong order in one or both of the files)

                if t1 != t2 {
                    handle_different_event_times(
                        &should_stop,
                        &comparison_result,
                        &mut event_count,
                        &events1,
                        &events2,
                    );
                    // wake up the reader threads
                    barrier.wait();
                    break;
                }

                // if time is smaller than last time, then events are not in chronological order
                if let Some(last_t) = last_time
                    && (t1 < last_t || t2 < last_t)
                {
                    error!(
                        "Events are not in chronological order: \
                            Event number {} has time {} in both files, but previous event has time \
                            {}.",
                        event_count + 1,
                        t1,
                        last_t
                    );
                    let mut result = comparison_result.lock().unwrap();
                    *result = Err(EventsFileNotEqualError::NotChronologicalOrder);
                    should_stop.store(true, AtomicOrdering::Relaxed);

                    // wake up the reader threads
                    barrier.wait();
                    break;
                }

                // If times are good, finally compare the batches of events
                match compare_batch_of_events(&events1, &events2) {
                    Ok(()) => {
                        event_count += events1.len() as u32;
                        last_time = Some(t1);
                    }
                    Err(id) => {
                        handle_missing_event(&should_stop, &comparison_result, &events1, id);
                        // wake up the reader threads
                        barrier.wait();
                        break;
                    }
                }
            }
            _ => {
                error!("Unexpected state: batch without time but not finished");
                should_stop.store(true, AtomicOrdering::Relaxed);
                barrier.wait();
                break;
            }
        }

        // wake up the reader threads to let them read the next batches
        barrier.wait();
    }
}

fn handle_missing_event(
    should_stop: &Arc<AtomicBool>,
    comparison_result: &Arc<Mutex<Result<(), EventsFileNotEqualError>>>,
    events1: &[Box<dyn EventTrait>],
    id: usize,
) {
    error!(
        "Events do not match. Event #{} is missing in file 2: {:?}",
        id + 1, // id is 0-indexed, so add 1 to count as humans do
        events1[id],
    );
    let mut result = comparison_result.lock().unwrap();
    *result = Err(EventsFileNotEqualError::MissingEvent {
        event: format!("{:?}", events1[id]),
    });
    should_stop.store(true, AtomicOrdering::Relaxed);
}

fn handle_different_event_times(
    should_stop: &Arc<AtomicBool>,
    comparison_result: &Arc<Mutex<Result<(), EventsFileNotEqualError>>>,
    event_count: &mut u32,
    events1: &Vec<Box<dyn EventTrait>>,
    events2: &Vec<Box<dyn EventTrait>>,
) {
    error!(
        "Event files differ starting from event #{event_count}. Times are different. Different events: \n    In file 1: {:?}\n    In file 2: {:?}; ",
        events1.first().unwrap(),
        events2.first().unwrap()
    );
    // times differ
    let mut result = comparison_result.lock().unwrap();
    *result = Err(EventsFileNotEqualError::DifferentEventTimes);
    should_stop.store(true, AtomicOrdering::Relaxed);
}

fn extract_events(batch1: Arc<Mutex<EventBatch>>) -> (Option<u32>, Vec<Box<dyn EventTrait>>, bool) {
    let (time1, events1, finished1) = {
        let mut b = batch1.lock().unwrap();
        let time = b.time;
        let events = std::mem::take(&mut b.events);
        let finished = b.finished;
        (time, events, finished)
    };
    (time1, events1, finished1)
}

fn compare_batch_of_events(
    event_batch_1: &Vec<Box<dyn EventTrait>>,
    event_batch_2: &Vec<Box<dyn EventTrait>>,
) -> Result<(), usize> {
    let mut seen_ids = std::collections::HashSet::new();

    for (id1, event1) in event_batch_1.iter().enumerate() {
        let mut event1_has_match = false;

        for (id2, event2) in event_batch_2.iter().enumerate() {
            if seen_ids.contains(&id2) {
                continue;
            }

            if event1 == event2 {
                event1_has_match = true;
                seen_ids.insert(id2);
                break;
            }
        }

        if !event1_has_match {
            return Err(id1); // return 0-indexed id of event in batch 1 for which no match was found in batch 2
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::simulation::events::utils::*;
    use macros::integration_test;

    #[integration_test]
    fn test_compare_identical_xml_event_files() {
        let file1 = "./tests/resources/events/expected_events.xml";
        let file2 = "./tests/resources/events/expected_events.xml";

        match compare_xml_event_files(file1, file2) {
            Ok(()) => (),
            Err(e) => panic!("Compared two identical files, but got error: {e}"),
        }
    }

    #[integration_test]
    fn test_compare_equiv_but_diff_xml_event_files() {
        let file1 = "./tests/resources/events/expected_events.xml";
        // Here, the order of two events with same time was changed, which is legal and should not cause an error
        let file2 = "./tests/resources/events/expected_events_changed_order_legally.xml";

        match compare_xml_event_files(file1, file2) {
            Ok(()) => (),
            Err(e) => panic!(
                "Compared two equivalent files (with same events but different order), but got error: {e}"
            ),
        }
    }

    #[integration_test]
    fn test_compare_xml_different_time_xml_event_files() {
        let file1 = "./tests/resources/events/expected_events.xml";
        // in this file, the order of the events was changed (illegally), so that the event with
        // time 32408 comes before the event with time 32400.
        // Therefore, we should get a DifferentEventTimes error in line 1
        let file2 = "./tests/resources/events/expected_events_changed_order_illegally.xml";

        match compare_xml_event_files(file1, file2) {
            Ok(()) => {
                panic!("Compared two files with one of them not in chronological order, but got Ok")
            }
            Err(e) => match e {
                EventsFileNotEqualError::DifferentEventTimes => {}

                _ => panic!(
                    "Compared two files where event times differ in line 1, but got a different error: {e}"
                ),
            },
        }
    }

    #[integration_test]
    fn test_compare_incorrectly_ordered_xml_event_files() {
        // Here, we compare the file with incorrect (not chronological) order to itself, so that we
        // should get a NotChronologicalOrder error in line 2.
        // (In the test above, we got a DifferentEventTimes error in line 1, since we compared to a
        // file which was chronologically ordered)
        let file1 = "./tests/resources/events/expected_events_changed_order_illegally.xml";
        let file2 = "./tests/resources/events/expected_events_changed_order_illegally.xml";

        match compare_xml_event_files(file1, file2) {
            Ok(()) => {
                panic!(
                    "Compared a file with incorrect (not chronological) order to itself, but got Ok"
                )
            }
            Err(e) => match e {
                EventsFileNotEqualError::NotChronologicalOrder => {
                    // expected error, do nothing
                }

                _ => panic!(
                    "Compared a file with incorrect (not chronological) order to itself, but got an unexpected error: {e}"
                ),
            },
        }
    }

    #[integration_test]
    fn test_compare_xml_event_files_w_data_mismatch() {
        let file1 = "./tests/resources/events/expected_events.xml";

        // In this file, "100_car" was changed to "101_car" in all events in which it occurs (first time in the 4th event with time 32409)
        let file2 = "./tests/resources/events/expected_events_modified_data.xml";

        match compare_xml_event_files(file1, file2) {
            Ok(()) => {
                panic!(
                    "Compared two files where the name of a car was changed in file2, but got Ok"
                )
            }
            Err(e) => match e {
                EventsFileNotEqualError::MissingEvent { event } => {
                    assert_eq!(
                        event,
                        String::from(
                            "PersonEntersVehicleEvent { time: 32409, person: 100, vehicle: 100_car, attributes: InternalAttributes { attributes: {} } }"
                        )
                    );
                }

                _ => panic!(
                    "Compared two files where the name of a car was changed in file2, but got an unexpected error: {e}"
                ),
            },
        }
    }

    #[integration_test]
    fn test_compare_xml_event_files_w_different_number_of_events() {
        let file1 = "./tests/resources/events/expected_events.xml";
        // Here, the last line was removed, so that file2 has one event less than file1.
        let file2 = "./tests/resources/events/expected_events_removed_events.xml";

        match compare_xml_event_files(file1, file2) {
            Ok(()) => {
                panic!(
                    "Compared two files where one file has fewer events than the other, but got Ok"
                )
            }
            Err(e) => match e {
                EventsFileNotEqualError::DifferentNumberOfEvents => {
                    // expected error, do nothing
                }

                _ => panic!(
                    "Compared two files where one file has fewer events than the other, but got an unexpected error: {e}"
                ),
            },
        }
    }
}
