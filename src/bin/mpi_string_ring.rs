use mpi::traits::{Communicator, CommunicatorCollectives, Destination, Source};
use mpi::Rank;

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let size = world.size();
    let rank = world.rank();

    let version = mpi::environment::library_version().unwrap();
    println!("Library Version: {version}");

    assert!(
        size > 1,
        "We need more than one process to pass messages around"
    );

    let next_rank = (rank + 1) % size;

    // process 0 starts
    if rank == 0 {
        let encoded_msg = encoded_message(&rank);
        println!("Process #{rank} sending first message.");
        world.process_at_rank(next_rank).send(&encoded_msg);
    }

    // every process should receive a message at some point
    let (received_encoded_msg, status) = world.any_process().receive_vec::<u8>();
    let received_msg = String::from_utf8(received_encoded_msg).unwrap();
    let sender = status.source_rank();
    println!("Process #{rank} received {received_msg} from Process #{sender}");

    // every process should pass on another message to its neighbor. If we have circled back to process
    // 0, we stop passing messages.
    if rank != 0 {
        let encoded_msg = encoded_message(&rank);
        world.process_at_rank(next_rank).send(&encoded_msg);
    }

    println!("Process #{rank} at barrier");
    world.barrier();
    println!("Process #{rank} after barrier. Done.");
}

fn encoded_message(rank: &Rank) -> Vec<u8> {
    let message = format!("I am Process #{rank}");
    Vec::from(message)
}
