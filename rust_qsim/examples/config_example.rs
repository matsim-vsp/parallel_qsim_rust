use rust_qsim::simulation::config::Config;

fn main() {
    let mut config = Config::default();
    config.simulation_mut().stuck_threshold = 42;
    print!("{:?}", config);
}
