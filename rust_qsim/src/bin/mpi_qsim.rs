#[cfg(feature = "mpi")]
use rust_qsim::simulation::controller::mpi_controller;

#[cfg(feature = "mpi")]
fn main() {
    mpi_controller::run_mpi();
}
