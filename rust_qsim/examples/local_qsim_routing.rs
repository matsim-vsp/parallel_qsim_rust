use clap::Parser;
use rust_qsim::external_services::routing::RoutingServiceAdapterFactory;
use rust_qsim::external_services::{AdapterHandleBuilder, AsyncExecutor, ExternalServiceType};
use rust_qsim::simulation::config::Config;
use rust_qsim::simulation::controller;
use rust_qsim::simulation::controller::local_controller::LocalControllerBuilder;
use rust_qsim::simulation::controller::ExternalServices;
use rust_qsim::simulation::logging::init_std_out_logging_thread_local;
use rust_qsim::simulation::scenario::GlobalScenario;
use std::sync::{Arc, Barrier};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct RoutingCommandLineArgs {
    #[arg(long, short)]
    router_ip: String,
    #[clap(flatten)]
    delegate: rust_qsim::simulation::config::CommandLineArgs,
}

fn main() {
    let _guard = init_std_out_logging_thread_local();
    let args = RoutingCommandLineArgs::parse();
    let config = Arc::new(Config::from(args.delegate));

    // Creating the routing adapter is only one task, so we add 1 and not the number of worker threads!
    let total_thread_count = config.partitioning().num_parts + 1;
    let barrier = Arc::new(Barrier::new(total_thread_count as usize));

    let factory = RoutingServiceAdapterFactory::new(vec![&args.router_ip], config.clone());

    let executor = AsyncExecutor::from_config(&config, barrier.clone());

    let (router_handle, send, send_sd) = executor.spawn_thread("router", factory);

    let mut services = ExternalServices::default();
    services.insert(ExternalServiceType::Routing("pt".into()), send.into());

    let scenario = GlobalScenario::build(config);

    let controller = LocalControllerBuilder::default()
        .global_scenario(scenario)
        .external_services(services)
        .global_barrier(barrier)
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
