use macros::integration_test;
use rust_qsim::simulation::events::utils;
use rust_qsim::simulation::events::utils::convert_proto_to_xml_events;
use rust_qsim::simulation::id;
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

    match utils::compare_xml_event_files(generated_file, expected_file) {
        Ok(()) => (),
        Err(e) => panic!(
            "Generated XML event file ('file 1') and expected event file ('file 2') differ: {e}"
        ),
    }
}
