use clap::Parser;
use rust_q_sim::external_services::routing::RoutingServiceAdapter;
use rust_q_sim::external_services::{AdapterHandleBuilder, ExternalServiceType};
use rust_q_sim::simulation::config::Config;
use rust_q_sim::simulation::controller;
use rust_q_sim::simulation::controller::local_controller::run_channel;
use rust_q_sim::simulation::controller::ExternalServices;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct CommandLineArgs {
    router_ip: String,
    config: PathBuf,
}

fn main() {
    let args = CommandLineArgs::parse();

    let (router_handle, send, send_sd) =
        RoutingServiceAdapter::new(&args.router_ip).as_thread("router");

    let mut services = ExternalServices::default();
    services.insert(ExternalServiceType::Routing("pt".into()), send.into());

    let config = Config::from(args.config);
    let sim_handles = run_channel(config, Default::default(), services);

    controller::try_join(
        sim_handles,
        vec![AdapterHandleBuilder::default()
            .shutdown_sender(send_sd)
            .handle(router_handle)
            .build()
            .unwrap()],
    )
}
