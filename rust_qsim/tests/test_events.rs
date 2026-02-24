use macros::integration_test;
use rust_qsim::simulation::events::utils::convert_proto_to_xml_events;
use rust_qsim::simulation::id;
use rust_qsim::simulation::io::proto::xml_events::XmlEventsReader;
use std::fs;
use std::path::{Path, PathBuf};

#[integration_test(rust_qsim)]
fn test_proto_to_xml() {
    // run proto2xml on results from a run of 3-links-config-2.yml
    let path_to_proto_files = "./tests/resources/3-links/".to_string();
    let output_folder = "./test_output/simulation/execute_3_links_2_parts/".to_string();
    let id_store = "./tests/resources/3-links/3-links.ids.binpb".to_string();
    let num_parts = 2;

    // create result directory, move the generated .gz file there
    fs::create_dir_all(&output_folder).unwrap();

    id::load_from_file(&PathBuf::from(id_store));
    convert_proto_to_xml_events(
        path_to_proto_files,
        num_parts,
        PathBuf::from(output_folder).join("events.xml.gz"),
    );

    // Load and compare two XML event files
    let generated_file =
        Path::new("./test_output/simulation/execute_3_links_2_parts/events.xml.gz");
    let expected_file = Path::new("./tests/resources/3-links/expected_events.xml");

    compare_xml_event_files(generated_file, expected_file);
}

/// Compares two XML event files event by event. Panics if any events differ or if the files have
/// different numbers of events.
fn compare_xml_event_files(file1: &Path, file2: &Path) {
    let mut reader1 = XmlEventsReader::new(file1);
    let mut reader2 = XmlEventsReader::new(file2);

    let mut line_count = 0;
    loop {
        let event1 = reader1.read_next();
        let event2 = reader2.read_next();

        match (event1, event2) {
            (Some((time1, event_data1)), Some((time2, event_data2))) => {
                assert_eq!(
                    time1, time2,
                    "Event times differ for event in line {}",
                    line_count
                );

                assert_eq!(
                    &event_data1, &event_data2,
                    "Event data differs for event in line {}",
                    line_count
                );

                line_count += 1;
            }
            (None, None) => {
                println!(
                    "✓ Successfully compared {} lines from both files",
                    line_count
                );
                break;
            }
            (Some(_), None) => {
                panic!(
                    "File1 has more events than file2 (file2 ended at event {})",
                    line_count
                );
            }
            (None, Some(_)) => {
                panic!(
                    "File2 has more events than file1 (file1 ended at event {})",
                    line_count
                );
            }
        }
    }
}
