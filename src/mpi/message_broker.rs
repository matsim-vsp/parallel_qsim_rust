use crate::mpi::messages::proto::Vehicle;
use mpi::topology::SystemCommunicator;

pub trait MessageBroker {
    fn send(&mut self, now: u32);
    fn receive(&mut self, now: u32);
    fn add_veh(&mut self, vehicle: Vehicle);
}
pub struct MpiMessageBroker {
    communicator: SystemCommunicator,
    neighbors: Vec<usize>,
}

impl MessageBroker for MpiMessageBroker {
    fn send(&mut self, now: u32) {
        todo!()
    }

    fn receive(&mut self, now: u32) {
        todo!()
    }

    fn add_veh(&mut self, vehicle: Vehicle) {
        todo!()
    }
}
