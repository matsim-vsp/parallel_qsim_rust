use rust_q_sim::config::Config;
use rust_q_sim::controller;
use rust_q_sim::logging::init_logging;

#[test]
fn three_link_network() {
    let config = Config::builder()
        .network_file(String::from("./assets/3-links/3-links-network.xml"))
        .population_file(String::from("./assets/3-links/1-agent.xml"))
        .output_dir(String::from(
            "./test_output/controller_it/three_link_network",
        ))
        .num_parts(3)
        .build();

    let _logger_guard = init_logging(&config.output_dir);

    controller::run(config);

    // somehow test the output
    println!("Done");
}
