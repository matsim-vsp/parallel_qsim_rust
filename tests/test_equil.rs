mod test_simulation;
use test_simulation::execute_sim_with_channels;
use rust_q_sim::simulation::config::CommandLineArgs;

#[test]
fn test_equil_scenario() {
    let args = CommandLineArgs {
        config_path: "assets/equil/equil-config.yml".to_string(),
        num_parts:   Some(1),
    };
    execute_sim_with_channels(args, "tests/resources/equil/expected_events.xml");
}