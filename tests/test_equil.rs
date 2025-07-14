use rust_q_sim::external_services::routing::{
    InternalRoutingRequest, InternalRoutingRequestPayload, InternalRoutingResponse,
};
use rust_q_sim::external_services::{
    execute_adapter, AdapterHandle, AdapterHandleBuilder, ExternalServiceType, RequestAdapter,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;

mod test_simulation;
use crate::test_simulation::TestExecutorBuilder;
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::controller::{ExternalServices, RequestSender};
use rust_q_sim::simulation::id::{store_to_file, Id};
use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::{InternalPlanElement, Population, PREPLANNING_HORIZON};
use rust_q_sim::simulation::vehicles::garage::Garage;

fn create_resources<F>(out_dir: &PathBuf, pop_adaption: F)
where
    F: Fn(&mut Population),
{
    let input_dir = PathBuf::from("./assets/equil/");
    let net = Network::from_file_as_is(&input_dir.join("equil-network.xml"));
    let mut garage = Garage::from_file(&input_dir.join("equil-vehicles.xml"));
    let mut pop = Population::from_file(&input_dir.join("equil-1-plan.xml"), &mut garage);

    pop_adaption(&mut pop);

    store_to_file(&out_dir.join("ids.binpb"));
    net.to_file(&out_dir.join("equil-network.binpb"));
    pop.to_file(&out_dir.join("equil-1-plan.binpb"));
    garage.to_file(&out_dir.join("equil-vehicles.binpb"));
}

#[test]
fn execute_equil_single_part() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_single_part/");
    create_resources(&test_dir, |_pop| {});

    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1.yml");

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events("./tests/resources/equil/expected_events.xml")
        .build()
        .unwrap()
        .execute();
}

#[test]
fn execute_equil_2_parts() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_with_channels/");
    create_resources(&test_dir, |_| {});

    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2.yml");

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events("./tests/resources/equil/expected_events.xml")
        .build()
        .unwrap()
        .execute();
}

#[test]
#[should_panic]
fn execute_equil_adaptive_planning_single_part_panics() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_single_part_adaptive/");
    let config_path = "./tests/resources/equil/equil-config-1-adaptive.yml".to_string();
    let expected_events = "./tests/resources/equil/expected_events.xml";

    // panics because no external service is provided
    execute_adaptive(
        test_dir,
        config_path,
        expected_events,
        ExternalServices::default(),
        vec![],
    );
}

#[test]
fn execute_equil_adaptive_planning_single_part() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_single_part_adaptive/");
    let config_path = "./tests/resources/equil/equil-config-1-adaptive.yml".to_string();
    let expected_events = "./tests/resources/equil/expected_events.xml";

    let (tx, rx) = mpsc::channel(10000);
    let (shutdown_send, shutdown_recv) = tokio::sync::watch::channel(false);

    let adapter = MockRoutingAdapter::default();

    let routing_thread = thread::Builder::new()
        .name("routing_adapter".to_string())
        .spawn(move || execute_adapter(rx, adapter, shutdown_recv))
        .unwrap();

    let mut map: HashMap<ExternalServiceType, RequestSender> = HashMap::new();
    map.insert(
        ExternalServiceType::Routing("car".to_string()),
        Arc::new(tx).into(),
    );

    execute_adaptive(
        test_dir,
        config_path,
        expected_events,
        map.into(),
        vec![AdapterHandleBuilder::default()
            .handle(routing_thread)
            .shutdown_sender(shutdown_send)
            .build()
            .unwrap()],
    );
}

#[test]
fn execute_equil_adaptive_planning_two_parts() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_with_channels-adaptive/");
    let config_path = "./tests/resources/equil/equil-config-2-adaptive.yml".to_string();
    let expected_events = "./tests/resources/equil/expected_events.xml";

    let (tx, rx) = mpsc::channel(10000);
    let (shutdown_send, shutdown_recv) = tokio::sync::watch::channel(false);

    let adapter = MockRoutingAdapter::default();

    let routing_thread = thread::Builder::new()
        .name("routing_adapter".to_string())
        .spawn(move || execute_adapter(rx, adapter, shutdown_recv))
        .unwrap();

    let mut map: HashMap<ExternalServiceType, RequestSender> = HashMap::new();
    map.insert(
        ExternalServiceType::Routing("car".to_string()),
        Arc::new(tx).into(),
    );

    execute_adaptive(
        test_dir,
        config_path,
        expected_events,
        map.into(),
        vec![AdapterHandleBuilder::default()
            .handle(routing_thread)
            .shutdown_sender(shutdown_send)
            .build()
            .unwrap()],
    );
}

#[derive(Default)]
struct MockRoutingAdapter {
    requests: Vec<InternalRoutingRequestPayload>,
}

impl RequestAdapter<InternalRoutingRequest> for MockRoutingAdapter {
    async fn on_request(&mut self, req: InternalRoutingRequest) {
        self.requests.push(req.payload);
        req.response_tx
            .send(InternalRoutingResponse::default())
            .unwrap();
    }

    fn on_shutdown(&mut self) {
        assert_eq!(self.requests.len(), 1);
        assert_eq!(
            self.requests[0],
            InternalRoutingRequestPayload {
                person_id: "1".to_string(),
                from_link: "1".to_string(),
                to_link: "20".to_string(),
                mode: "car".to_string(),
                departure_time: 21600,
                now: 21000,
            }
        );
    }
}

fn execute_adaptive(
    test_dir: PathBuf,
    config_path: String,
    expected_events: &str,
    map: ExternalServices,
    adapter_handles: Vec<AdapterHandle>,
) {
    let f = |pop: &mut Population| {
        let agent = pop.persons.get_mut(&Id::create("1")).unwrap();
        let plan = agent.selected_plan_mut();
        match plan.elements.get_mut(1).unwrap() {
            InternalPlanElement::Activity(_) => {
                panic!()
            }
            InternalPlanElement::Leg(l) => {
                // add a preplanning horizon attribute to
                l.attributes.insert(PREPLANNING_HORIZON, 10 * 60);
            }
        }
    };

    create_resources(&test_dir, f);

    let config_args = CommandLineArgs::new_with_path(config_path);

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events(expected_events)
        .external_services(map)
        .adapter_handles(adapter_handles)
        .build()
        .unwrap()
        .execute();
}
