use std::path::PathBuf;

use clap::Parser;
use rust_qsim::simulation::events::utils;
use rust_qsim::simulation::id;
use rust_qsim::simulation::logging::init_std_out_logging_thread_local;
use tracing::info;

/// merges proto events from multiple files into a single XML file
fn main() {
    let _g = init_std_out_logging_thread_local();
    let args = InputArgs::parse();
    info!("Proto2Xml with args: {args:?}");

    info!("Load Id Store");
    id::load_from_file(&PathBuf::from(&args.id_store));

    let output_file_path = PathBuf::from(&args.path).join("events.xml.gz");

    utils::convert_proto_to_xml_events(args.path, args.num_parts, output_file_path);
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
