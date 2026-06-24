mod test_simulation;
use crate::test_simulation::TestExecutorBuilder;

use std::sync::{Arc};
use macros::integration_test;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::config::ScoringPlansCollectionType::{HomeSending, Mapping};

#[integration_test(rust_qsim)]
fn test_scoring_single_part_backpacking() {
    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1-scoring.yml");
    let config = Arc::new(Config::from_args(config_args));

    TestExecutorBuilder::default()
        .config(config)
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
fn test_scoring_single_part_homesending() {
    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1-scoring.yml");
    let mut config = Config::from_args(config_args);
    config.scoring_mut().plans_collection_type = HomeSending;

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
fn test_scoring_single_part_mapping() {
    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1-scoring.yml");
    let mut config = Config::from_args(config_args);
    config.scoring_mut().plans_collection_type = Mapping;

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
fn test_scoring_2_parts_backpacking() {
    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2-scoring.yml");
    let config = Arc::new(Config::from_args(config_args));

    TestExecutorBuilder::default()
        .config(config)
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
fn test_scoring_2_parts_homesending() {
    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2-scoring.yml");
    let mut config = Config::from_args(config_args);
    config.scoring_mut().plans_collection_type = HomeSending;

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
fn test_scoring_2_parts_mapping() {
    let config_args = CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2-scoring.yml");
    let mut config = Config::from_args(config_args);
    config.scoring_mut().plans_collection_type = Mapping;
    config.scoring_mut().collector_threads = 2;

    TestExecutorBuilder::default()
        .config(Arc::new(config))
        .expected_events(None)
        .build()
        .unwrap()
        .execute();
}
