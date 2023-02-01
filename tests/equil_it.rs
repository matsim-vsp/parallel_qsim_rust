use crate::event_test_utils::{compare_events, run_mpi_simulation_and_convert_events};
use serial_test::serial;

mod event_test_utils;
#[test]
#[serial]
fn test_equil_1() {
    test_equil(1)
}

#[test]
#[serial]
fn test_equil_2() {
    test_equil(2)
}
#[test]
#[serial]
fn test_equil_5() {
    test_equil(5)
}

fn test_equil(parts: usize) {
    let output_dir = format!("test_output/mpi_test/equil_scenario/{}/", parts);
    run_mpi_simulation_and_convert_events(
        parts,
        "assets/equil/equil-network.xml",
        "assets/equil/equil-plans.xml.gz",
        output_dir.as_str(),
        "use-plans",
    );
    compare_events(output_dir.as_str(), "tests/resources/equil")
}
