#[cfg(feature = "mpi")]
use rust_q_sim::simulation::controller::mpi_controller;

#[cfg(feature = "mpi")]
fn main() {
    mpi_controller::run_mpi();
}
