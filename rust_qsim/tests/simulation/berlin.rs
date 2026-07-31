use crate::support::simulation_executor::TestExecutorBuilder;
use macros::deterministic_id_test;
use rust_qsim::simulation::config::Config;
use std::path::PathBuf;
use std::sync::Arc;

#[deterministic_id_test(rust_qsim)]
fn test_berlin_1() {
    test_berlin(1);
}

#[deterministic_id_test(rust_qsim)]
fn test_berlin_2() {
    test_berlin(2);
}

fn test_berlin(parts: u32) {
    let mut config = Config::from_path("./assets/berlin-v6.4/config.yml");
    config.partitioning_mut().num_parts = parts;
    config.output_mut().output_dir = PathBuf::from(format!(
        "./test_output/simulation/berlin-v6.4-0.1pct-{}",
        parts
    ));

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(Some(
            "./tests/resources/berlin-v6.4-0.1pct/events.0.xml.zst",
        ))
        .build()
        .unwrap()
        .execute();
}
