use rust_q_sim::config::Config;
use rust_q_sim::controller;
use rust_q_sim::logging::init_logging;

#[test]
fn equil_scenario() {
    let config = Config::builder()
        .network_file(String::from("./assets/equil/equil-network.xml"))
        .population_file(String::from("./assets/equil/equil-plans.xml.gz"))
        .output_dir(String::from("./test_output/controller_it/equil_scenario"))
        .num_parts(2)
        .start_time(0)
        .end_time(86400)
        .build();

    let _logger_guard = init_logging(&config.output_dir, None);

    controller::run(config);
}
