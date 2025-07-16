use clap::Parser;
use rust_q_sim::external_services::routing::RoutingServiceAdapterFactory;
use rust_q_sim::external_services::{AdapterHandleBuilder, ExternalServiceType};
use rust_q_sim::simulation::config::Config;
use rust_q_sim::simulation::controller;
use rust_q_sim::simulation::controller::local_controller::run_channel;
use rust_q_sim::simulation::controller::ExternalServices;
use rust_q_sim::simulation::logging::init_std_out_logging;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct RoutingCommandLineArgs {
    #[arg(long, short)]
    router_ip: String,
    #[clap(flatten)]
    delegate: rust_q_sim::simulation::config::CommandLineArgs,
}

fn main() {
    let _guard = init_std_out_logging();
    let args = RoutingCommandLineArgs::parse();
    let config = Config::from(args.delegate);

    let (router_handle, send, send_sd) =
        RoutingServiceAdapterFactory::new(&args.router_ip, config.clone()).spawn_thread("router");

    let mut services = ExternalServices::default();
    services.insert(ExternalServiceType::Routing("pt".into()), send.into());

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
