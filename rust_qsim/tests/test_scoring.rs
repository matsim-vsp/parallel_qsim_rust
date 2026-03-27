mod test_simulation;
use crate::test_simulation::TestExecutorBuilder;

use std::sync::Arc;
use macros::integration_test;
use rust_qsim::simulation::config::{CommandLineArgs, Config};

#[integration_test(rust_qsim)]
fn test_scoring() {
    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2.yml");
    let config = Arc::new(Config::from_args(config_args));

    TestExecutorBuilder::default()
        .config(config)
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}
