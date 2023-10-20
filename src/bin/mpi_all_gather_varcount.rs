use mpi::datatype::PartitionMut;
use mpi::traits::{Communicator, CommunicatorCollectives};
use mpi::Count;

use rust_q_sim::simulation::messaging::messages::proto::TravelTimesMessage;

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();

    // construct messages
    let mut message = TravelTimesMessage::new();
    message.add_travel_time(world.rank() as u64, world.rank() as u32 * 2);
    if world.rank() == 2 {
        message.add_travel_time(1000000u64, 40000u32)
    }

    let send_traffic_info = message.serialize();

    // send lengths
    let len_of_traffic_info = send_traffic_info.len() as i32;
    let mut all_len_of_traffic_info = vec![0i32; world.size() as usize];
    world.all_gather_into(&len_of_traffic_info, &mut all_len_of_traffic_info[..]);

    let mut all_traffic_info = vec![0u8; all_len_of_traffic_info.iter().sum::<i32>() as usize];

    let displs: Vec<Count> = all_len_of_traffic_info
        .iter()
        .scan(0, |acc, &x| {
            let tmp = *acc;
            *acc += x;
            Some(tmp)
        })
        .collect();

    let lengths = all_len_of_traffic_info.clone();

    // send real messages
    {
        let mut partition =
            PartitionMut::new(&mut all_traffic_info, all_len_of_traffic_info, &displs[..]);
        world.all_gather_varcount_into(&send_traffic_info[..], &mut partition);
    }

    // slice over byte vector and deserialize
    let mut last_end_index = 0usize;
    for len in lengths {
        let begin_index = last_end_index;
        let end_index = last_end_index + len as usize;
        println!(
            "Process {:?} gathered info between[{},{}]: {:?}.",
            world.rank(),
            begin_index,
            end_index,
            TravelTimesMessage::deserialize(&all_traffic_info[begin_index..end_index])
        );
        last_end_index = end_index;
    }
}
