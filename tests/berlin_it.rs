use rust_q_sim::config::Config;
use rust_q_sim::controller;
use rust_q_sim::logging::init_logging;

#[test]
fn equil_scenario() {
    let config = Config::builder()
        .network_file(String::from(
            "/home/janek/test-files/berlin-test-network-no-pt.xml.gz",
        ))
        .population_file(String::from(
            "/home/janek/test-files/berlin-10pct-all-plans-no-pt.xml.gz",
        ))
        .output_dir(String::from("/home/janek/test-files/berlin-output"))
        .num_parts(14)
        .start_time(0)
        .end_time(86400)
        .sample_size(0.1)
        .build();

    let _logger_guard = init_logging(&config.output_dir);

    controller::run(config);
}
