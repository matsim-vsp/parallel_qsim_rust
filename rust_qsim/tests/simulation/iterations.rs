use crate::support::simulation_executor::TestExecutorBuilder;
use macros::deterministic_id_test;
use rust_qsim::simulation::config::{CommandLineArgs, Config, WriteEvents};
use std::path::PathBuf;
use std::sync::Arc;

#[deterministic_id_test(rust_qsim)]
fn equil_single_part_runs_10_iterations() {
    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1.yml");
    let mut config = Config::from_args(config_args);
    config.simulation_mut().first_iteration = 0;
    config.simulation_mut().last_iteration = 9;
    config.output_mut().output_dir =
        PathBuf::from("./test_output/simulation/equil_single_part_10_iterations");

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}

#[deterministic_id_test(rust_qsim)]
fn equil_single_part_writes_events_at_interval_and_last_iteration() {
    let output_dir = PathBuf::from("./test_output/simulation/equil_event_interval");
    let mut config = Config::from_args(CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-1.yml",
    ));
    config.simulation_mut().first_iteration = 0;
    config.simulation_mut().last_iteration = 5;
    config.simulation_mut().write_events_interval = 3;
    config.output_mut().write_events = WriteEvents::XmlGz;
    config.output_mut().output_dir = output_dir.clone();

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();

    let iters_dir = output_dir.join("ITERS");
    assert!(!iters_dir.join("it.0").join("events").exists());
    assert!(!iters_dir.join("it.1").join("events").exists());
    assert!(!iters_dir.join("it.2").join("events").exists());
    assert!(
        iters_dir
            .join("it.3")
            .join("events")
            .join("events.0.xml.gz")
            .exists()
    );
    assert!(!iters_dir.join("it.4").join("events").exists());
    assert!(
        iters_dir
            .join("it.5")
            .join("events")
            .join("events.0.xml.gz")
            .exists()
    );
}

#[deterministic_id_test(rust_qsim)]
fn equil_single_part_writes_events_for_single_last_iteration() {
    let output_dir = PathBuf::from("./test_output/simulation/equil_event_interval_single_last");
    let mut config = Config::from_args(CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-1.yml",
    ));
    config.simulation_mut().first_iteration = 0;
    config.simulation_mut().last_iteration = 0;
    config.simulation_mut().write_events_interval = 50;
    config.output_mut().write_events = WriteEvents::XmlGz;
    config.output_mut().output_dir = output_dir.clone();

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();

    assert!(
        output_dir
            .join("ITERS")
            .join("it.0")
            .join("events")
            .join("events.0.xml.gz")
            .exists()
    );
}

#[deterministic_id_test(rust_qsim)]
fn equil_single_part_write_events_none_creates_no_iteration_events() {
    let output_dir = PathBuf::from("./test_output/simulation/equil_event_interval_none");
    let mut config = Config::from_args(CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-1.yml",
    ));
    config.simulation_mut().first_iteration = 0;
    config.simulation_mut().last_iteration = 3;
    config.simulation_mut().write_events_interval = 1;
    config.output_mut().write_events = WriteEvents::None;
    config.output_mut().output_dir = output_dir.clone();

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();

    for iteration in 0..=3 {
        assert!(
            !output_dir
                .join("ITERS")
                .join(format!("it.{iteration}"))
                .join("events")
                .exists()
        );
    }
}

#[deterministic_id_test(rust_qsim)]
fn equil_single_part_writes_plans_at_interval_and_last_iteration() {
    let output_dir = PathBuf::from("./test_output/simulation/equil_plan_interval");
    let mut config = Config::from_args(CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-1.yml",
    ));
    config.simulation_mut().first_iteration = 0;
    config.simulation_mut().last_iteration = 5;
    config.simulation_mut().write_plans_interval = 3;
    config.output_mut().write_events = WriteEvents::None;
    config.output_mut().output_dir = output_dir.clone();

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();

    let iters_dir = output_dir.join("ITERS");
    assert!(!iters_dir.join("it.0").join("output_plans.xml.gz").exists());
    assert!(!iters_dir.join("it.1").join("output_plans.xml.gz").exists());
    assert!(!iters_dir.join("it.2").join("output_plans.xml.gz").exists());
    assert!(iters_dir.join("it.3").join("output_plans.xml.gz").exists());
    assert!(!iters_dir.join("it.4").join("output_plans.xml.gz").exists());
    assert!(iters_dir.join("it.5").join("output_plans.xml.gz").exists());
    assert!(output_dir.join("output_plans.xml.gz").exists());
}

#[deterministic_id_test(rust_qsim)]
fn equil_single_part_writes_plans_for_single_last_iteration() {
    let output_dir = PathBuf::from("./test_output/simulation/equil_plan_interval_single_last");
    let mut config = Config::from_args(CommandLineArgs::new_with_path(
        "./tests/resources/equil/equil-config-1.yml",
    ));
    config.simulation_mut().first_iteration = 0;
    config.simulation_mut().last_iteration = 0;
    config.simulation_mut().write_plans_interval = 50;
    config.output_mut().write_events = WriteEvents::None;
    config.output_mut().output_dir = output_dir.clone();

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();

    assert!(
        output_dir
            .join("ITERS")
            .join("it.0")
            .join("output_plans.xml.gz")
            .exists()
    );
}
