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
    let _g = init_std_out_logging_thread_local();
    let args = InputArgs::parse();
    info!("Proto2Xml with args: {args:?}");

    info!("Load Id Store");
    id::load_from_file(&PathBuf::from(args.id_store));

    let mut manager = EventsManager::new();
    let output_file_path = PathBuf::from(&args.path).join("events.xml.gz");
    let register_xml_writer = XmlEventsWriter::registrator(output_file_path.clone());

    register_xml_writer(&mut manager);

    read_proto_events(
        &mut manager,
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
