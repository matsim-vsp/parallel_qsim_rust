use mpi::traits::{Communicator, CommunicatorCollectives};
use rust_q_sim::simulation::messaging::messages::proto::TravelTimesMessage;

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();

    let mut message = TravelTimesMessage::new();
    message.add_travel_time(world.rank() as u64 + 0, (world.rank() as u32 + 0) * 2);

    let send_traffic_info = message.serialize();
    println!(
        "Process {:?} traffic info length: {:?}",
        world.rank(),
        send_traffic_info.len()
    );
    let count = world.size() as usize;
    let mut gathered_vector = vec![0u8; send_traffic_info.len() * count];

    world.all_gather_into(&send_traffic_info, &mut gathered_vector[..]);
    println!(
        "Process {:?} Length of gathered: {:?}",
        world.rank(),
        gathered_vector.len()
    );

    for m in gathered_vector.chunks(send_traffic_info.len()) {
        println!(
            "Process {:?} gathered sequence: {:?}.",
            world.rank(),
            TravelTimesMessage::deserialize(m)
        );
    }
}
