use crate::event_test_utils::{compare_events, run_mpi_simulation_and_convert_events};
use serial_test::serial;

mod event_test_utils;

#[test]
#[serial]
fn test_adhoc_routing() {
    let output_dir = "test_output/mpi_test/adhoc_routing/";
    run_mpi_simulation_and_convert_events(
        2,
        "assets/adhoc_routing/network.xml",
        "assets/adhoc_routing/agents_no_leg.xml",
        output_dir,
        "ad-hoc",
    );
    compare_events(output_dir, "tests/resources/adhoc_routing")
}
