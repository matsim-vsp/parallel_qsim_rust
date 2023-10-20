use std::thread::sleep;
use std::time::{Duration, Instant};

use mpi::traits::{Communicator, CommunicatorCollectives, Destination, Source};

use rust_q_sim::simulation::messaging::messages::proto::ExperimentalMessage;

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let rank = world.rank();
    let size = world.size();

    let version = mpi::environment::library_version().unwrap();
    println!("Library Version: {version}");

    let next_rank = (rank + 1) % size;
    let start = Instant::now();

    // process 0 starts
    if rank == 0 {
        let message: ExperimentalMessage = ExperimentalMessage {
            timestamp: start.elapsed().as_nanos() as u64,
            counter: 0,
            additional_message: String::from("Test string"),
        };
        let buf = message.serialize();
        world.process_at_rank(next_rank).send(&buf);
    }

    // every process should receive a message at some point
    let (received_encoded_msg, _status) = world.any_process().receive_vec::<u8>();
    let mut received_msg = ExperimentalMessage::deserialize(&received_encoded_msg);
    println!(
        "Process #{rank} received counter: {}, add_msg: {} at time: {}",
        received_msg.counter, received_msg.additional_message, received_msg.timestamp
    );

    if rank != 0 {
        sleep(Duration::from_secs(2));
        received_msg.timestamp = start.elapsed().as_nanos() as u64;
        received_msg.counter += 1;
        received_msg.additional_message = format!("This is coming from Process #{rank}");
        let buf = received_msg.serialize();
        world.process_at_rank(next_rank).send(&buf);
    }

    println!("Process #{rank} at barrier");
    world.barrier();
    println!("Process #{rank} after barrier. Done.");
}
