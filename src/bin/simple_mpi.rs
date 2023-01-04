use mpi::traits::Communicator;

fn main() {
    let universe = mpi::initialize().unwrap();

    let bla = mpi::environment::version();
    println!("MPI version: {:?}", bla);
    let world = universe.world();
    println!(
        "Hello parallel world from process {} of {}!",
        world.rank(),
        world.size()
    );
}
