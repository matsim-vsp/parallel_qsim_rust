use mpi::traits::Communicator;

fn main() {
    let universe = mpi::initialize().unwrap();
    let rank = universe.world().rank();
    let size = universe.world().size();

    if rank == 0 {
        println!("{}", mpi::environment::library_version().unwrap());
    }
    println!("mpi_print_info: Process {rank}/{size}.");
}
