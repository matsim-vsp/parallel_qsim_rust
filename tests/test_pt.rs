use crate::test_simulation::TestExecutorBuilder;
use rust_q_sim::external_services::routing::RoutingServiceAdapterFactory;
use rust_q_sim::external_services::{AdapterHandleBuilder, ExternalServiceType};
use rust_q_sim::simulation::config::{CommandLineArgs, Config};
use rust_q_sim::simulation::controller::ExternalServices;
use rust_q_sim::simulation::id::store_to_file;
use rust_q_sim::simulation::messaging::events::EventsSubscriber;
use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::Population;
use rust_q_sim::simulation::pt::TransitSchedule;
use rust_q_sim::simulation::vehicles::garage::Garage;
use std::collections::HashMap;
use std::path::PathBuf;

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

#[test]
fn test_pt_tutorial() {
    let test_dir = PathBuf::from("./test_output/simulation/pt_tutorial/");
    create_resources(&test_dir, &PathBuf::from("plans_1.xml.gz"));

    let config_args =
        CommandLineArgs::new_with_path("./tests/resources/pt_tutorial/pt_tutorial_config.yml");

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events(Some("./tests/resources/pt_tutorial/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}

#[test]
// #[ignore]
// to be tested with running routing service;
// --config /Users/paulh/git/parallel_qsim_rust/assets/pt_tutorial/config.xml --output output/v6.4/test-router
fn test_pt_adaptive() {
    let test_dir = PathBuf::from("./test_output/simulation/pt_tutorial_adaptive/");
    create_resources(&test_dir, &PathBuf::from("plans_1-dummy.xml"));

    let mut config_args = CommandLineArgs::new_with_path(
        "./tests/resources/pt_tutorial/pt_tutorial_config_adaptive.yml",
    );

    config_args
        .overrides
        .push((String::from("routing.mode"), String::from("ad-hoc")));

    let (handle, send, shutdown) = RoutingServiceAdapterFactory::new(
        "http://localhost:50051",
        Config::from(config_args.clone()),
    )
    .spawn_thread("routing_adapter");

    let mut services = ExternalServices::default();
    services.insert(ExternalServiceType::Routing("pt".into()), send.into());

    let subs: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>> = HashMap::new();
    // subs.insert(0, vec![Box::new(XmlEventsWriter::new("test.xml".as_ref()))]);

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events(Some(
            "./tests/resources/pt_tutorial/expected_events_adaptive.xml",
        ))
        .external_services(services)
        .additional_subscribers(subs)
        .adapter_handles(vec![AdapterHandleBuilder::default()
            .shutdown_sender(shutdown)
            .handle(handle)
            .build()
            .unwrap()])
        .build()
        .unwrap()
        .execute();
}
