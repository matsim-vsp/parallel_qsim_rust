use std::path::PathBuf;

use clap::Parser;
use rust_qsim::simulation::events::utils;
use rust_qsim::simulation::id;
use tracing::info;

/// merges proto events from multiple files into a single XML file
fn main() {
    let args = InputArgs::parse();
    info!("Proto2Xml with args: {args:?}");

    info!("Load Id Store");
    id::load_from_file(&PathBuf::from(&args.id_store));

    utils::convert_proto_to_xml_events(args.path, args.num_parts);
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
