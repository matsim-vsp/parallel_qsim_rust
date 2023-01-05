use log::error;
use mpi::traits::{Communicator, CommunicatorCollectives, Destination, Root, Source};
use std::string::FromUtf8Error;

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let size = world.size();
    let rank = world.rank();

    let next_rank = (rank + 1) % size;
    // let previous_rank = (rank - 1 + size) % size;

    let message = Vec::from("bla bla bla");
    if rank == 0 {
        println!("Process #{} will send to rank #{}", rank, next_rank);
        world.process_at_rank(next_rank).send(&message);
        world.this_process().broadcast_into()
    } else if rank == 1 {
        println!("Process #{} about to receive.", rank);

        // receive_vec actually uses MProbe and MatchReceived internally initializing
        // a buffer with the appropriate size.
        let (buffer, status) = world.any_process().receive_vec::<u8>();

        println!(
            "Process #{} has received message: {:?} with status {:?}",
            rank, message, status
        );

        match String::from_utf8(buffer) {
            Ok(message) => {
                println!("Reconstructed message: {}", message)
            }
            Err(_) => {
                error!("Could not not reconstruct message :-(")
            }
        };
    }

    println!("Process #{} about to wait at barrier", rank);
    world.barrier();
    println!("Process #{} after barrier", rank);
    /*
    if rank == 0 {
        // the 0th process should start the message chain
        println!("Process 0 sending: {}", message);
        world.process_at_rank(next_rank).send(message.as_bytes());
    } else if rank == 1 {
        let mut receive_buffer: Vec<u8> = Vec::with_capacity(message.len());
        println!("Process {} is about to receive a message", rank);
        let status = world.any_process().receive_into(&mut receive_buffer);
        println!(
            "Process 1 received: {:?} with status {:?}",
            receive_buffer, status
        );
    }

     */
}
