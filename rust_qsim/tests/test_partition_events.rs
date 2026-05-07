use macros::integration_test;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::framework_events::{
    AgentEntersPartitionEvent, AgentLeavesPartitionEvent, EventOrigin, PartitionEvent,
    PartitionListenerRegisterFn, PartitionRuntimeEvent, VehicleEntersPartitionEvent,
    VehicleLeavesPartitionEvent,
};
use rust_qsim::simulation::scenario::MutableScenario;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

#[integration_test(rust_qsim)]
fn network_route_emits_partition_events() {
    let events = collect_partition_events("./tests/resources/3-links/3-links-config-2.yml");
    assert_eq!(4, events.len(), "unexpected partition events: {:?}", events);

    assert_has_partition_handoff(
        &events,
        0,
        PartitionEvent::VehicleLeavesPartition(VehicleLeavesPartitionEvent {
            vehicle_id: rust_qsim::simulation::id::Id::get_from_ext("100_car"),
            to: 1,
            time: 0,
        }),
    );
    assert_has_partition_handoff(
        &events,
        0,
        PartitionEvent::AgentLeavesPartition(AgentLeavesPartitionEvent {
            agent_id: rust_qsim::simulation::id::Id::get_from_ext("100"),
            to: 1,
            time: 0,
        }),
    );
    assert_has_partition_handoff(
        &events,
        1,
        PartitionEvent::VehicleEntersPartition(VehicleEntersPartitionEvent {
            vehicle_id: rust_qsim::simulation::id::Id::get_from_ext("100_car"),
            from: 0,
            time: 0,
        }),
    );
    assert_has_partition_handoff(
        &events,
        1,
        PartitionEvent::AgentEntersPartition(AgentEntersPartitionEvent {
            agent_id: rust_qsim::simulation::id::Id::get_from_ext("100"),
            from: 0,
            time: 0,
        }),
    );
}

#[integration_test(rust_qsim)]
fn teleported_route_emits_partition_events() {
    let events =
        collect_partition_events("./tests/resources/3-links/3-links-config-2-teleport.yml");
    assert_eq!(4, events.len(), "unexpected partition events: {:?}", events);

    assert_has_partition_handoff(
        &events,
        0,
        PartitionEvent::VehicleLeavesPartition(VehicleLeavesPartitionEvent {
            vehicle_id: rust_qsim::simulation::id::Id::get_from_ext("100_walk"),
            to: 1,
            time: 0,
        }),
    );
    assert_has_partition_handoff(
        &events,
        0,
        PartitionEvent::AgentLeavesPartition(AgentLeavesPartitionEvent {
            agent_id: rust_qsim::simulation::id::Id::get_from_ext("100"),
            to: 1,
            time: 0,
        }),
    );
    assert_has_partition_handoff(
        &events,
        1,
        PartitionEvent::VehicleEntersPartition(VehicleEntersPartitionEvent {
            vehicle_id: rust_qsim::simulation::id::Id::get_from_ext("100_walk"),
            from: 0,
            time: 0,
        }),
    );
    assert_has_partition_handoff(
        &events,
        1,
        PartitionEvent::AgentEntersPartition(AgentEntersPartitionEvent {
            agent_id: rust_qsim::simulation::id::Id::get_from_ext("100"),
            from: 0,
            time: 0,
        }),
    );
}

fn collect_partition_events(config_path: &str) -> Vec<PartitionRuntimeEvent> {
    let config = Arc::new(Config::from_args(CommandLineArgs::new_with_path(
        config_path,
    )));
    let scenario = MutableScenario::load(config.clone());
    let (sender, receiver) = channel::<PartitionRuntimeEvent>();

    let mut listeners: HashMap<u32, Vec<Box<PartitionListenerRegisterFn>>> = HashMap::new();
    for rank in 0..config.partitioning().num_parts {
        listeners.insert(rank, vec![create_partition_listener(sender.clone())]);
    }
    drop(sender);

    let controller = ControllerBuilder::default_with_scenario(scenario)
        .partition_event_register_fn(listeners)
        .build()
        .unwrap();

    controller.run();

    receiver.try_iter().collect()
}

fn create_partition_listener(
    sender: Sender<PartitionRuntimeEvent>,
) -> Box<PartitionListenerRegisterFn> {
    Box::new(move |events| {
        events.on_event(move |event| {
            sender
                .send(event.clone())
                .expect("failed to collect partition event in test");
        });
    })
}

fn assert_has_partition_handoff(
    events: &[PartitionRuntimeEvent],
    origin_partition: u32,
    payload: PartitionEvent,
) {
    assert!(
        events.iter().any(|event| {
            event.meta.origin == EventOrigin::Partition(origin_partition)
                && event.meta.iteration == 0
                && partition_payload_matches(&event.payload, &payload)
        }),
        "missing partition event {:?} from partition {} in {:?}",
        payload,
        origin_partition,
        events
    );
}

fn partition_payload_matches(actual: &PartitionEvent, expected: &PartitionEvent) -> bool {
    match (actual, expected) {
        (
            PartitionEvent::VehicleLeavesPartition(actual),
            PartitionEvent::VehicleLeavesPartition(expected),
        ) => actual.vehicle_id == expected.vehicle_id && actual.to == expected.to,
        (
            PartitionEvent::AgentLeavesPartition(actual),
            PartitionEvent::AgentLeavesPartition(expected),
        ) => actual.agent_id == expected.agent_id && actual.to == expected.to,
        (
            PartitionEvent::VehicleEntersPartition(actual),
            PartitionEvent::VehicleEntersPartition(expected),
        ) => actual.vehicle_id == expected.vehicle_id && actual.from == expected.from,
        (
            PartitionEvent::AgentEntersPartition(actual),
            PartitionEvent::AgentEntersPartition(expected),
        ) => actual.agent_id == expected.agent_id && actual.from == expected.from,
        _ => false,
    }
}
