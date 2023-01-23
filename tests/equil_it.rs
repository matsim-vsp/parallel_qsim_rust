use crate::event_test_utils::run_simulation_and_compare_events;
use rust_q_sim::config::Config;
use serial_test::serial;

mod event_test_utils;

#[test]
#[serial]
fn equil_scenario_one_thread() {
    let config = get_config(1);
    run_simulation_and_compare_events(config, "tests/resources/equil")
}

#[test]
#[serial]
fn equil_scenario_two_threads() {
    let config = get_config(2);
    run_simulation_and_compare_events(config, "tests/resources/equil")
}

#[test]
#[serial]
fn equil_scenario_five_threads() {
    let config = get_config(5);
    run_simulation_and_compare_events(config, "tests/resources/equil")
}

fn get_config(num_parts: usize) -> Config {
    Config::builder()
        .network_file(String::from("./assets/equil/equil-network.xml"))
        .population_file(String::from("./assets/equil/equil-plans.xml.gz"))
        .output_dir(format!(
            "./test_output/controller_it/equil_scenario/{}parts",
            num_parts
        ))
        .num_parts(num_parts)
        .start_time(0)
        .end_time(86400)
        .build()
}
