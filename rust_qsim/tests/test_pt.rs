use crate::test_simulation::TestExecutorBuilder;
use macros::integration_test;
use rust_qsim::external_services::routing::RoutingServiceAdapterFactory;
use rust_qsim::external_services::{AdapterHandleBuilder, AsyncExecutor, ExternalServiceType};
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::ExternalServices;
use rust_qsim::simulation::events::OnEventFnBuilder;
use rust_qsim::simulation::pt::TransitSchedule;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

mod test_simulation;

#[integration_test(rust_qsim)]
fn test_pt_tutorial() {
    TransitSchedule::from_file(&PathBuf::from("./assets/pt_tutorial/").join("transitschedule.xml"));

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
fn pt_adaptive_with_access_egress() {
    test_pt_adaptive(PathBuf::from(
        "./assets/pt_tutorial/plans_1-access_egress.xml",
    ))
}

#[integration_test(rust_qsim)]
#[ignore]
fn pt_adaptive_with_dummy() {
    test_pt_adaptive(PathBuf::from("./assets/pt_tutorial/plans_1-dummy.xml"))
}

// to be tested with running routing service;
// --config /Users/paulh/git/parallel_qsim_rust/rust_qsim/assets/pt_tutorial/config.xml --output output/v6.4/test-router
fn test_pt_adaptive(pop_path: PathBuf) {
    let mut config_args = CommandLineArgs::new_with_path(
        "./tests/resources/pt_tutorial/pt_tutorial_config_adaptive.yml",
    );

    config_args
        .overrides
        .push((String::from("routing.mode"), String::from("ad-hoc")));

    let c = Config::from(config_args);
    c.population().path = pop_path;

    let config = Arc::new(c);

    let total_thread_count = config.partitioning().num_parts + 1;
    let global_barrier = Arc::new(Barrier::new(total_thread_count as usize));

    let executor = AsyncExecutor::from_config(&config, global_barrier.clone());

    let routing_factory = RoutingServiceAdapterFactory::new(
        vec!["http://localhost:50051"],
        config.clone(),
        executor.shutdown_handles(),
    );

    let (handle, send, shutdown) = executor.spawn_thread("routing_adapter", routing_factory);

    let mut services = ExternalServices::default();
    services.insert(ExternalServiceType::Routing("pt".into()), send.into());

    let subs: HashMap<u32, Vec<Box<OnEventFnBuilder>>> = HashMap::new();
    // subs.insert(
    //     0,
    //     vec![Box::new(XmlEventsWriter::register("test.xml".into()))],
    // );

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
