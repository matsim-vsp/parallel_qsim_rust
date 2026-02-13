use std::path::PathBuf;

use clap::Parser;
use rust_qsim::simulation::events::utils::read_proto_events;
use rust_qsim::simulation::events::EventsManager;
use rust_qsim::simulation::id;
use rust_qsim::simulation::io::proto::xml_events::XmlEventsWriter;
use rust_qsim::simulation::logging::init_std_out_logging_thread_local;
use tracing::info;

/// merges proto events from multiple files into a single XML file
fn main() {
    let args = InputArgs::parse();
    info!("Proto2Xml with args: {args:?}");
    run(args);
}

fn run(args: InputArgs) {
    let _g = init_std_out_logging_thread_local();

    info!("Load Id Store");
    id::load_from_file(&PathBuf::from(args.id_store));

    let mut publisher = EventsManager::new();
    let output_file_path = PathBuf::from(&args.path).join("events.xml.gz");
    let register_xml_writer = XmlEventsWriter::register(output_file_path.clone());

    register_xml_writer(&mut publisher);

    read_proto_events(
        &mut publisher,
        &PathBuf::from(args.path),
        String::from("events"),
        args.num_parts,
    );
    info!(
        "Finished writing to xml file ({}).",
        output_file_path.display()
    );
}

#[derive(Parser, Debug)]
struct InputArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub id_store: String,
    #[arg(long, default_value_t = 1)]
    pub num_parts: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    // use flate2::bufread;
    use flate2::bufread::GzDecoder;
    use rust_qsim::simulation::io::proto::xml_events::XmlEventsReader;
    use std::fs::File;
    use std::io::copy;
    use std::io::BufRead;
    use std::io::BufReader;
    use std::path::Path;

    #[test]
    fn test_output() {
        // run proto2xml on results from a run of 3-links-config-2.yml
        let args = InputArgs {
            path: "./test_output/simulation/execute_3_links_2_parts/".to_string(),
            id_store: "./test_output/simulation/execute_3_links_2_parts/test3links2parts.ids.binpb"
                .to_string(),
            num_parts: 2,
        };
        run(args);

        // decompress the generated XML file and save as .xml
        let compressed_path = "./test_output/simulation/execute_3_links_2_parts/events.xml.gz";
        let decompressed_path = "./test_output/simulation/execute_3_links_2_parts/events.xml";
        decompress_gz(compressed_path, decompressed_path);

        // Load and compare two XML event files
        let generated_file =
            Path::new("./test_output/simulation/execute_3_links_2_parts/events.xml");
        let expected_file = Path::new("./tests/resources/3-links/expected_events.xml");

        compare_xml_event_files_as_string(generated_file, expected_file);

        // commented out, since the XmlEventsReader doesn't know the event type "vehicle enters
        // traffic", which is present in the given xml files, and thus panics.
        // compare_xml_event_files_as_xml(generated_file, expected_file);
    }

    /// Decompresses .gz file into a new file
    fn decompress_gz(compressed_path: &str, decompressed_path: &str) {
        let input = BufReader::new(File::open(compressed_path).unwrap());
        let mut output = File::create(decompressed_path).unwrap();

        let mut decoder = GzDecoder::new(input);
        copy(&mut decoder, &mut output).unwrap();
        info!(
            "Finished unzipping {} to {}.",
            compressed_path, decompressed_path
        );
    }

    /// Compares two XML event files line by line as strings, ignoring leading and trailing
    /// whitespace. Panics if any line differs or if the files have different number of lines.
    fn compare_xml_event_files_as_string(file1: &Path, file2: &Path) {
        let reader1 = BufReader::new(File::open(file1).unwrap());
        let reader2 = BufReader::new(File::open(file2).unwrap());

        let mut line_iterator1 = reader1.lines().map(|l| l.unwrap());
        let mut line_iterator2 = reader2.lines().map(|l| l.unwrap());

        let mut line_count = 0;
        loop {
            let line1 = line_iterator1.next();
            let line2 = line_iterator2.next();

            match (line1, line2) {
                (Some(line_content1), Some(line_content2)) => {
                    assert_eq!(
                        line_content1.trim(),
                        line_content2.trim(),
                        "Line content differs at line {}",
                        line_count
                    );
                    line_count += 1;
                }
                (None, None) => {
                    println!(
                        "✓ Successfully compared {} events from both files",
                        line_count
                    );
                    break;
                }
                (Some(_), None) => {
                    panic!(
                        "File1 has more lines than file2 (file2 ended at line {})",
                        line_count
                    );
                }
                (None, Some(_)) => {
                    panic!(
                        "File2 has more lines than file1 (file1 ended at line {})",
                        line_count
                    );
                }
            }
        }
    }

    /// Compares two XML event files event by event. Panics if any events differ or if the files have different number of events.
    fn compare_xml_event_files_as_xml(file1: &Path, file2: &Path) {
        let mut reader1 = XmlEventsReader::new(file1);
        let mut reader2 = XmlEventsReader::new(file2);

        let mut line_count = 0;
        loop {
            // this panics for the given xml files, since the XmlEventsReader doesn't know the event
            // type "vehicle enters traffic"
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
                        event_data1.attributes(),
                        event_data2.attributes(),
                        "Event data differs for event in line {}",
                        line_count
                    );

                    // to be removed, just to see what attributes are being compared
                    for attr in event_data1.attributes().iter() {
                        info!("File1 Event Attr: {} = {:?}", attr.0, attr.1);
                    }
                    for attr in event_data2.attributes().iter() {
                        info!("File2 Event Attr: {} = {:?}", attr.0, attr.1);
                    }

                    line_count += 1;
                }
                (None, None) => {
                    println!(
                        "✓ Successfully compared {} events from both files",
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
}
