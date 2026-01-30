use crate::test_simulation::TestExecutorBuilder;
use macros::integration_test;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use std::sync::Arc;

mod test_simulation;

#[integration_test(rust_qsim)]
fn execute_3_links_single_part() {
    let config_args =
        CommandLineArgs::new_with_path("./tests/resources/3-links/3-links-config-1.yml");

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from(config_args)))
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

#[integration_test(rust_qsim)]
fn execute_3_links_2_parts() {
    let config_args =
        CommandLineArgs::new_with_path("./tests/resources/3-links/3-links-config-2.yml");

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from(config_args)))
        .expected_events(Some("./tests/resources/3-links/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}
