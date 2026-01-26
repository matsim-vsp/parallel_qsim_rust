use macros::integration_test;
use rust_qsim::external_services::routing::{
    InternalRoutingRequest, InternalRoutingRequestPayload, InternalRoutingResponse,
};
use rust_qsim::external_services::{
    AdapterHandle, AdapterHandleBuilder, AsyncExecutor, ExternalServiceType, RequestAdapter,
    RequestAdapterFactory,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

mod test_simulation;
use crate::test_simulation::TestExecutorBuilder;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::{ExternalServices, RequestSender};
use rust_qsim::simulation::id::{store_to_file, Id};
use rust_qsim::simulation::network::Network;
use rust_qsim::simulation::population::{InternalPlanElement, Population, PREPLANNING_HORIZON};
use rust_qsim::simulation::vehicles::garage::Garage;

fn create_resources<F>(out_dir: &Path, pop_adaption: F)
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

#[integration_test(rust_qsim)]
fn execute_equil_single_part() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_single_part/");
    create_resources(&test_dir, |_pop| {});

    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1.yml");
    let config = Arc::new(Config::from(config_args));

    TestExecutorBuilder::default()
        .config(config)
        .expected_events(Some("./tests/resources/equil/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
fn execute_equil_2_parts() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_with_channels/");
    create_resources(&test_dir, |_| {});

    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2.yml");
    let config = Arc::new(Config::from(config_args));

    TestExecutorBuilder::default()
        .config(config)
        .expected_events(Some("./tests/resources/equil/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
#[should_panic]
fn execute_equil_adaptive_planning_single_part_panics() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_single_part_adaptive/");
    let config_path = "./tests/resources/equil/equil-config-1-adaptive.yml".to_string();
    let expected_events = "./tests/resources/equil/expected_events.xml";

    // panics because no external service is provided
    execute_adaptive(
        test_dir,
        Config::from(CommandLineArgs::new_with_path(config_path)),
        expected_events,
        ExternalServices::default(),
        vec![],
        Arc::new(Barrier::new(1)),
    );
}

#[integration_test(rust_qsim)]
fn execute_equil_adaptive_planning_single_part() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_single_part_adaptive/");
    let config_path = "./tests/resources/equil/equil-config-1-adaptive.yml".to_string();
    let expected_events = "./tests/resources/equil/expected_events.xml";

    let mock_routing_adapter = MockRoutingAdapterFactory::default();

    let config = Config::from(CommandLineArgs::new_with_path(config_path));

    let parts = config.partitioning().num_parts + 1;
    let barrier = Arc::new(Barrier::new(parts as usize));
    let executor = AsyncExecutor::from_config(&config, barrier.clone());

    let (handle, send, shutdown) = executor.spawn_thread("routing_adapter", mock_routing_adapter);

    let mut map: HashMap<ExternalServiceType, RequestSender> = HashMap::new();
    map.insert(
        ExternalServiceType::Routing("car".to_string()),
        Arc::new(send).into(),
    );

    execute_adaptive(
        test_dir,
        config,
        expected_events,
        map.into(),
        vec![AdapterHandleBuilder::default()
            .handle(handle)
            .shutdown_sender(shutdown)
            .build()
            .unwrap()],
        barrier,
    );
}

#[integration_test(rust_qsim)]
fn execute_equil_adaptive_planning_two_parts() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_with_channels-adaptive/");
    let config_path = "./tests/resources/equil/equil-config-2-adaptive.yml".to_string();
    let expected_events = "./tests/resources/equil/expected_events.xml";

    let mock_routing_adapter = MockRoutingAdapterFactory::default();

    let config = Config::from(CommandLineArgs::new_with_path(config_path));

    let barrier = Arc::new(Barrier::new((config.partitioning().num_parts + 1) as usize));
    let executor = AsyncExecutor::from_config(&config, barrier.clone());

    let (handle, send, shutdown) = executor.spawn_thread("routing_adapter", mock_routing_adapter);

    let mut map: HashMap<ExternalServiceType, RequestSender> = HashMap::new();
    map.insert(
        ExternalServiceType::Routing("car".to_string()),
        Arc::new(send).into(),
    );

    execute_adaptive(
        test_dir,
        config,
        expected_events,
        map.into(),
        vec![AdapterHandleBuilder::default()
            .handle(handle)
            .shutdown_sender(shutdown)
            .build()
            .unwrap()],
        barrier,
    );
}

#[derive(Default)]
struct MockRoutingAdapterFactory {}

impl RequestAdapterFactory<InternalRoutingRequest> for MockRoutingAdapterFactory {
    async fn build(self) -> impl RequestAdapter<InternalRoutingRequest> {
        MockRoutingAdapter::default()
    }
}

#[derive(Default)]
struct MockRoutingAdapter {
    requests: Vec<InternalRoutingRequestPayload>,
}

impl RequestAdapter<InternalRoutingRequest> for MockRoutingAdapter {
    fn on_request(&mut self, req: InternalRoutingRequest) {
        self.requests.push(req.payload);
        req.response_tx
            .send(InternalRoutingResponse::default())
            .unwrap();
    }

    fn on_shutdown(&mut self) {
        assert_eq!(self.requests.len(), 1);
        assert!(
            self.requests[0].equals_ignoring_uuid(&InternalRoutingRequestPayload {
                person_id: "1".to_string(),
                from_link: "1".to_string(),
                from_x: -25000.,
                from_y: 0.,
                to_link: "20".to_string(),
                to_x: 3456.,
                to_y: 4242.,
                mode: "car".to_string(),
                departure_time: 21600,
                now: 21000,
                uuid: Default::default(),
            })
        );
    }
}

fn execute_adaptive(
    test_dir: PathBuf,
    config: Config,
    expected_events: &str,
    map: ExternalServices,
    adapter_handles: Vec<AdapterHandle>,
    global_barrier: Arc<Barrier>,
) {
    let f = |pop: &mut Population| {
        let agent = pop.persons.get_mut(&Id::create("1")).unwrap();
        let plan = agent.selected_plan_mut();
        match plan.elements.get_mut(0).unwrap() {
            InternalPlanElement::Activity(a) => {
                // add a preplanning horizon attribute to
                a.attributes.insert(PREPLANNING_HORIZON, 10 * 60);
            }
            InternalPlanElement::Leg(_) => {
                panic!()
            }
        }
    };

    create_resources(&test_dir, f);

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(Some(expected_events))
        .external_services(map)
        .adapter_handles(adapter_handles)
        .global_barrier(global_barrier)
        .build()
        .unwrap()
        .execute();
}
