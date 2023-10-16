use std::collections::{BinaryHeap, HashMap, HashSet};

use log::info;
use mpi::topology::SystemCommunicator;
use mpi::traits::{Communicator, Destination, Source};
use mpi::Rank;

use crate::simulation::messaging::messages::proto::{Vehicle, VehicleMessage};
use crate::simulation::network::sim_network::SimNetworkPartition;

pub trait NetCommunicator {
    fn send_receive<F>(
        &self,
        vehicles: HashMap<u32, VehicleMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        on_msg: F,
    ) where
        F: FnMut(VehicleMessage);
}

pub struct DummyNetCommunicator();

impl NetCommunicator for DummyNetCommunicator {
    fn send_receive<F>(
        &self,
        _vehicles: HashMap<u32, VehicleMessage>,
        _expected_vehicle_messages: &mut HashSet<u32>,
        _now: u32,
        _on_msg: F,
    ) where
        F: FnMut(VehicleMessage),
    {
        info!("Dummy Net Communicator doesn't do anything.")
    }
}

pub struct MpiNetCommunicator {
    mpi_communicator: SystemCommunicator,
}

impl NetCommunicator for MpiNetCommunicator {
    fn send_receive<F>(
        &self,
        out_messages: HashMap<u32, VehicleMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        mut on_msg: F,
    ) where
        F: FnMut(VehicleMessage),
    {
        let buf_msg: Vec<_> = out_messages.values().map(|m| (m, m.serialize())).collect();

        // we have to use at least immediate send here. Otherwise we risk blocking on send as explained
        // in https://paperpile.com/app/p/e209e0b3-9bdb-08c7-8a62-b1180a9ac954 chapter 4.3, 4.4 and 4.12.
        // The underlying mpi-implementation may wait for the receiver to call a recv variant, and provide
        // a buffer, where the buffer used for the send operation can be written into. If process 1 and 2
        // want to send with MPI_Send, which is a blocking operation, both processes will wait, that
        // the other calls MPI_Recv, which never happens, because both processes are stuck at MPI_Send
        //
        // With immediate_send (MPI_Isend) we tell MPI that we are ready to send away the message buffer,
        // then the same process immediately calls MPI_Recv (blocking) which makes room for a message
        // buffer. In the case of the above example, both processes are calling MPI_Recv and provide
        // a buffer to write the message into, which was issued in MPI_Isend.
        //
        // The rsmpi library wraps non-blocking mpi-communication into a scope, so that the compiler
        // can ensure that a buffer is not moved while the request is in progress.
        mpi::request::multiple_scope(buf_msg.len(), |scope, reqs| {
            // ------- Send Part ---------
            for (message, buf) in buf_msg.iter() {
                let req = self
                    .mpi_communicator
                    .process_at_rank(message.to_process as Rank)
                    .immediate_send(scope, buf);
                reqs.add(req);
            }

            // Use blocking MPI_recv here, since we don't have anything to do if there are no other
            // messages.
            while !expected_vehicle_messages.is_empty() {
                let (encoded_msg, _status) = self.mpi_communicator.any_process().receive_vec();
                let msg = VehicleMessage::deserialize(&encoded_msg);
                let from_rank = msg.from_process;

                // If a message was received from a neighbor partition for this very time step, remove
                // that partition from expected messages which indicates which partitions we are waiting
                // for
                if msg.time == now {
                    expected_vehicle_messages.remove(&from_rank);
                }

                on_msg(msg);
            }

            // wait here, so that all requests finish. This is necessary, because a process might send
            // more messages than it receives. This happens, if a process sends messages to remote
            // partitions (teleported legs) but only receives messages from neighbor partitions.
            reqs.wait_all(&mut Vec::new());
        });
    }
}

pub struct NetMessageBroker<C>
where
    C: NetCommunicator,
{
    pub rank: Rank,
    //communicator: SystemCommunicator,
    communicator: C,
    out_messages: HashMap<u32, VehicleMessage>,
    in_messages: BinaryHeap<VehicleMessage>,
    // store link mapping with internal ids instead of id structs, because vehicles only store internal
    // ids (usize) and this way we don't need to keep a reference to the global network's id store
    link_mapping: HashMap<usize, usize>,
    neighbors: HashSet<u32>,
}

impl<C> NetMessageBroker<C>
where
    C: NetCommunicator,
{
    pub fn new_single_partition(
        network: &SimNetworkPartition,
    ) -> NetMessageBroker<DummyNetCommunicator> {
        let link_mapping = network
            .global_network
            .links
            .iter()
            .map(|link| (link.id.internal(), link.partition))
            .collect();

        NetMessageBroker {
            rank: 0,
            communicator: DummyNetCommunicator(),
            out_messages: Default::default(),
            in_messages: Default::default(),
            link_mapping,
            neighbors: Default::default(),
        }
    }

    pub fn new_mpi_broker(
        world: SystemCommunicator,
        network: &SimNetworkPartition,
    ) -> NetMessageBroker<MpiNetCommunicator> {
        let rank = world.rank();
        let neighbors = network
            .neighbors()
            .iter()
            .map(|rank| *rank as u32)
            .collect();
        let link_mapping = network
            .global_network
            .links
            .iter()
            .map(|link| (link.id.internal(), link.partition))
            .collect();

        let communicator = MpiNetCommunicator {
            mpi_communicator: world,
        };
        NetMessageBroker {
            rank,
            communicator,
            out_messages: HashMap::new(),
            in_messages: BinaryHeap::new(),
            link_mapping,
            neighbors,
        }
    }

    pub fn rank_for_link(&self, link_id: u64) -> u64 {
        *self.link_mapping.get(&(link_id as usize)).unwrap() as u64
    }

    pub fn add_veh(&mut self, vehicle: Vehicle, now: u32) {
        let link_id = vehicle.curr_link_id().unwrap();
        let partition = *self.link_mapping.get(&link_id).unwrap() as u32;
        let message = self
            .out_messages
            .entry(partition)
            .or_insert_with(|| VehicleMessage::new(now, self.rank as u32, partition));
        message.add(vehicle);
    }

    pub fn send_recv(&mut self, now: u32) -> Vec<VehicleMessage> {
        let vehicles = self.prepare_send_recv_vehicles(now);
        let mut result: Vec<VehicleMessage> = Vec::new();
        let mut expected_vehicle_messages = self.neighbors.clone();

        self.pop_from_cache(&mut expected_vehicle_messages, &mut result, now);

        let a = &self.communicator;
        let b = &mut self.in_messages;

        a.send_receive(vehicles, &mut expected_vehicle_messages, now, |msg| {
            Self::handle_incoming_msg(msg, &mut result, b, now)
        });

        result
    }

    fn handle_incoming_msg(
        msg: VehicleMessage,
        result: &mut Vec<VehicleMessage>,
        in_messages: &mut BinaryHeap<VehicleMessage>,
        now: u32,
    ) {
        if msg.time <= now {
            result.push(msg);
        } else {
            in_messages.push(msg);
        }
    }

    fn pop_from_cache(
        &mut self,
        expected_messages: &mut HashSet<u32>,
        messages: &mut Vec<VehicleMessage>,
        now: u32,
    ) {
        while let Some(msg) = self.in_messages.peek() {
            //  info!("; {}; {}; pop cache ", self.rank, now);
            if msg.time <= now {
                //  info!(
                //      "; {}; {now}; pop_cache; {}; {}; {}; {expected_messages:?}",
                //      self.rank, msg.from_process, msg.to_process, msg.time
                //  );
                expected_messages.remove(&msg.from_process);
                messages.push(self.in_messages.pop().unwrap())
            } else {
                //  info!("; {}; {}; break in cache ", self.rank, now);
                break; // important! otherwise this is an infinite loop
            }
        }
    }

    fn prepare_send_recv_vehicles(&mut self, now: u32) -> HashMap<u32, VehicleMessage> {
        let capacity = self.out_messages.len();
        let mut messages =
            std::mem::replace(&mut self.out_messages, HashMap::with_capacity(capacity));

        for partition in &self.neighbors {
            let neighbor_rank = *partition;
            messages
                .entry(neighbor_rank)
                .or_insert_with(|| VehicleMessage::new(now, self.rank as u32, neighbor_rank));
        }
        messages
    }
}

#[cfg(test)]
mod tests {
    use mpi::traits::Communicator;

    #[test]
    fn some_test() {
        let universe = mpi::initialize().unwrap();
        let rank = universe.world().rank();

        println!("This test was run!!! {}", rank)
    }
}
