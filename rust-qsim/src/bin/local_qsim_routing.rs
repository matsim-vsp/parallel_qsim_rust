use clap::Parser;
use rust_q_sim::external_services::routing::RoutingServiceAdapterFactory;
use rust_q_sim::external_services::{AdapterHandleBuilder, ExternalServiceType};
use rust_q_sim::simulation::config::Config;
use rust_q_sim::simulation::controller;
use rust_q_sim::simulation::controller::local_controller::LocalControllerBuilder;
use rust_q_sim::simulation::controller::ExternalServices;
use rust_q_sim::simulation::logging::init_std_out_logging_thread_local;
use rust_q_sim::simulation::scenario::GlobalScenario;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct RoutingCommandLineArgs {
    #[arg(long, short)]
    router_ip: String,
    #[clap(flatten)]
    delegate: rust_q_sim::simulation::config::CommandLineArgs,
}

fn main() {
    let _guard = init_std_out_logging_thread_local();
    let args = RoutingCommandLineArgs::parse();
    let config = Config::from(args.delegate);

    let (router_handle, send, send_sd) =
        RoutingServiceAdapterFactory::new(&args.router_ip, config.clone()).spawn_thread("router");

    let mut services = ExternalServices::default();
    services.insert(ExternalServiceType::Routing("pt".into()), send.into());

    let scenario = GlobalScenario::build(config);

    let controller = LocalControllerBuilder::default()
        .global_scenario(scenario)
        .external_services(services)
        .build()
        .unwrap();

    let sim_handles = controller.run();

    controller::try_join(
        sim_handles,
        vec![AdapterHandleBuilder::default()
            .shutdown_sender(send_sd)
            .handle(router_handle)
            .build()
            .unwrap()],
    )
}
