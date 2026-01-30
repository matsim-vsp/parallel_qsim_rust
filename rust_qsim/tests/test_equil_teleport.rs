mod test_simulation;

use crate::test_simulation::TestExecutorBuilder;
use macros::integration_test;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use std::sync::Arc;

// one agent having a network route, car being not a main mode => simulation should teleport the agent
#[integration_test(rust_qsim)]
fn teleport_network_route() {
    let config_args = CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-teleport-network-route.yml",
    );

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from(config_args)))
        .expected_events(Some(
            "./tests/resources/equil/expected_events_teleport_network_route.xml",
        ))
        .build()
        .unwrap()
        .execute();
}

// one agent having a generic route, car being not a main mode => simulation should teleport the agent
#[integration_test(rust_qsim)]
fn teleport_generic_route() {
    let config_args = CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-teleport-generic-route.yml",
    );

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from(config_args)))
        .expected_events(Some(
            "./tests/resources/equil/expected_events_teleport_generic_route.xml",
        ))
        .build()
        .unwrap()
        .execute();
}

// one agent having a network route, car being a main mode => already implemented
#[integration_test(rust_qsim)]
fn simulate_network_route() {
    let config_args = CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-simulate-network-route.yml",
    );

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from(config_args)))
        .expected_events(Some(
            "./tests/resources/equil/expected_events_simulate_network_route.xml",
        ))
        .build()
        .unwrap()
        .execute();
}

// one agent having a generic route, car being a main mode => simulation should crash
#[integration_test(rust_qsim)]
#[should_panic]
fn simulate_generic_route_panics() {
    let config_args = CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-simulate-generic-route-panics.yml",
    );

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from(config_args)))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}
