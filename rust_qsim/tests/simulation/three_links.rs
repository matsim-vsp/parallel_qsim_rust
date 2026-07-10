use crate::support::simulation_executor::TestExecutorBuilder;
use macros::deterministic_id_test;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use std::sync::Arc;

#[deterministic_id_test(rust_qsim)]
fn three_links_single_part_matches_expected_events() {
    let config_args =
        CommandLineArgs::new_with_path("./tests/resources/3-links/3-links-config-1.yml");

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from_args(config_args)))
        // .expected_events(None)
        // .additional_subscribers(HashMap::from([(
        //     0,
        //     vec![XmlEventsWriter::register("test_output/test.xml".into())],
        // )]))
        .expected_events(Some("./tests/resources/3-links/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}

#[deterministic_id_test(rust_qsim)]
fn three_links_two_parts_match_expected_events() {
    let config_args =
        CommandLineArgs::new_with_path("./tests/resources/3-links/3-links-config-2.yml");

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from_args(config_args)))
        .expected_events(Some("./tests/resources/3-links/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}
