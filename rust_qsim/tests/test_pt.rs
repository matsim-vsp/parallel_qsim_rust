use crate::test_simulation::TestExecutorBuilder;
use macros::integration_test;
use rust_qsim::external_services::routing::RoutingServiceAdapterFactory;
use rust_qsim::external_services::{AdapterHandleBuilder, AsyncExecutor, ExternalServiceType};
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::ExternalServices;
use rust_qsim::simulation::events::OnEventFnBuilder;
use rust_qsim::simulation::id::store_to_file;
use rust_qsim::simulation::network::Network;
use rust_qsim::simulation::population::Population;
use rust_qsim::simulation::pt::TransitSchedule;
use rust_qsim::simulation::vehicles::garage::Garage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

mod test_simulation;

fn create_resources(out_dir: &PathBuf, pop: &PathBuf) {
    let input_dir = PathBuf::from("./assets/pt_tutorial/");
    let net = Network::from_file_as_is(&input_dir.join("multimodalnetwork.xml"));
    let mut garage = Garage::from_file(&input_dir.join("vehicles.xml"));
    let pop = Population::from_file(&input_dir.join(pop), &mut garage);
    TransitSchedule::from_file(&input_dir.join("transitschedule.xml"));

    store_to_file(&out_dir.join("ids.binpb"));
    net.to_file(&out_dir.join("network.binpb"));
    pop.to_file(&out_dir.join("plans_1.binpb"));
    garage.to_file(&out_dir.join("vehicles.binpb"));
}

#[integration_test(rust_qsim)]
fn test_pt_tutorial() {
    let test_dir = PathBuf::from("./test_output/simulation/pt_tutorial/");
    create_resources(&test_dir, &PathBuf::from("plans_1.xml.gz"));

    let config = Arc::new(Config::from(CommandLineArgs::new_with_path(
        "./tests/resources/pt_tutorial/pt_tutorial_config.yml",
    )));

    TestExecutorBuilder::default()
        .config(config)
        .expected_events(Some("./tests/resources/pt_tutorial/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
#[ignore]
// to be tested with running routing service;
// --config /Users/paulh/git/parallel_qsim_rust/rust_qsim/assets/pt_tutorial/config.xml --output output/v6.4/test-router
fn test_pt_adaptive() {
    let test_dir = PathBuf::from("./test_output/simulation/pt_tutorial_adaptive/");
    create_resources(&test_dir, &PathBuf::from("plans_1-dummy.xml"));

    let mut config_args = CommandLineArgs::new_with_path(
        "./tests/resources/pt_tutorial/pt_tutorial_config_adaptive.yml",
    );

    config_args
        .overrides
        .push((String::from("routing.mode"), String::from("ad-hoc")));

    let config = Arc::new(Config::from(config_args));

    let total_thread_count = config.partitioning().num_parts + 1;
    let global_barrier = Arc::new(Barrier::new(total_thread_count as usize));

    let executor = AsyncExecutor::from_config(&config, global_barrier.clone());

    let routing_factory =
        RoutingServiceAdapterFactory::new(vec!["http://localhost:50051"], config.clone());

    let (handle, send, shutdown) = executor.spawn_thread("routing_adapter", routing_factory);

    let mut services = ExternalServices::default();
    services.insert(ExternalServiceType::Routing("pt".into()), send.into());

    let subs: HashMap<u32, Vec<Box<OnEventFnBuilder>>> = HashMap::new();
    // subs.insert(0, vec![Box::new(XmlEventsWriter::new("test.xml".as_ref()))]);

    TestExecutorBuilder::default()
        .config(config)
        .expected_events(Some(
            "./tests/resources/pt_tutorial/expected_events_adaptive.xml",
        ))
        .external_services(services)
        .additional_subscribers(subs)
        .global_barrier(global_barrier)
        .adapter_handles(vec![AdapterHandleBuilder::default()
            .shutdown_sender(shutdown)
            .handle(handle)
            .build()
            .unwrap()])
        .build()
        .unwrap()
        .execute();
}
