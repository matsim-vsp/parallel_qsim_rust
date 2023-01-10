use mpi::traits::{Communicator, CommunicatorCollectives, Destination, Source};

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let rank = world.rank();
    let size = world.size();
    let version = mpi::environment::library_version().unwrap();
    println!("Library Version: {version}");

    assert_eq!(
        2, size,
        "This example is supposed to be run with two processes. (mpirun -np 2...)"
    );
    let other_process = match rank {
        0 => 1,
        1 => 0,
        _ => {
            panic!("This shouldn't happen :-(")
        }
    };
    let mut counter = 0;

    // start the ping pong
    if rank == 0 {
        println!("Process #{rank} sending {counter}");
        world.process_at_rank(1).send(&counter);
    }

    // play ping pong
    while counter < 10 {
        let (message, _status) = world.any_process().receive::<i32>();
        println!("Process #{rank} received {message}");
        counter = message + 1;
        println!("Process #{rank} sending {counter}");
        world.process_at_rank(other_process).send(&counter);
    }

    println!("Process #{rank} at barrier.");
    world.barrier();
    println!("Process #{rank} after barrier. Done.");
}
