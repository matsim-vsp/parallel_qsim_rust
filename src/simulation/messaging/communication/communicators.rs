use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Barrier};

use mpi::collective::CommunicatorCollectives;
use mpi::point_to_point::{Destination, Source};
use mpi::topology::{Communicator, SimpleCommunicator};
use mpi::Rank;
use tracing::{info, instrument, span, Level};

use crate::simulation::wire_types::messages::{SimMessage, SyncMessage};

pub trait SimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        vehicles: HashMap<u32, SyncMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        on_msg: F,
    ) where
        F: FnMut(SyncMessage);

    fn barrier(&self);

    fn rank(&self) -> u32;
}

pub struct DummySimCommunicator();

impl SimCommunicator for DummySimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        _vehicles: HashMap<u32, SyncMessage>,
        _expected_vehicle_messages: &mut HashSet<u32>,
        _now: u32,
        _on_msg: F,
    ) where
        F: FnMut(SyncMessage),
    {
    }

    fn barrier(&self) {
        info!("Barrier was called on DummySimCommunicator, which doesn't do anything.")
    }

    fn rank(&self) -> u32 {
        0
    }
}

pub struct ChannelSimCommunicator {
    receiver: Receiver<SimMessage>,
    senders: Vec<Sender<SimMessage>>,
    rank: u32,
    barrier: Arc<Barrier>,
}

impl ChannelSimCommunicator {
    pub fn create_n_2_n(num_parts: u32) -> Vec<ChannelSimCommunicator> {
        let mut senders: Vec<_> = Vec::new();
        let mut comms: Vec<_> = Vec::new();
        let barrier = Arc::new(Barrier::new(num_parts as usize));

        for rank in 0..num_parts {
            let (sender, receiver) = channel();
            let comm = ChannelSimCommunicator {
                receiver,
                senders: vec![],
                rank,
                barrier: barrier.clone(),
            };
            senders.push(sender);
            comms.push(comm);
        }

        for comm in &mut comms {
            for sender in &senders {
                comm.senders.push(sender.clone());
            }
        }

        comms
    }

    pub fn rank(&self) -> u32 {
        self.rank
    }
}

impl SimCommunicator for ChannelSimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        vehicles: HashMap<u32, SyncMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        mut on_msg: F,
    ) where
        F: FnMut(SyncMessage),
    {
        // send messages to everyone
        for (target, msg) in vehicles {
            let sender = self.senders.get(target as usize).unwrap();
            sender
                .send(SimMessage::from_sync_message(msg))
                .expect("Failed to send vehicle message in message broker");
        }

        // receive messages from everyone
        while !expected_vehicle_messages.is_empty() {
            let received_msg = self
                .receiver
                .recv()
                .expect("Error while receiving messages")
                .sync_message();
            let from_rank = received_msg.from_process;

            // If a message was received from a neighbor partition for this very time step, remove
            // that partition from expected messages which indicates which partitions we are waiting
            // for
            if received_msg.time == now {
                expected_vehicle_messages.remove(&from_rank);
            }

            // publish the received message to the message broker
            on_msg(received_msg);
        }
    }

    fn barrier(&self) {
        self.barrier.wait();
    }

    fn rank(&self) -> u32 {
        self.rank
    }
}

pub struct MpiSimCommunicator {
    pub mpi_communicator: SimpleCommunicator,
}

impl SimCommunicator for MpiSimCommunicator {
    #[instrument(level = "trace", skip(self, on_msg), fields(rank = self.rank()))]
    fn send_receive_vehicles<F>(
        &self,
        out_messages: HashMap<u32, SyncMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        mut on_msg: F,
    ) where
        F: FnMut(SyncMessage),
    {
        let send_span = span!(Level::TRACE, "send_msgs", rank = self.rank(), now = now);
        let send_time = send_span.enter();
        let buf_msg: Vec<_> = out_messages
            .into_iter()
            .map(|(to, m)| (to, SimMessage::from_sync_message(m).serialize()))
            .collect();

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
            for (to, buf) in buf_msg.iter() {
                let req = self
                    .mpi_communicator
                    .process_at_rank(*to as Rank)
                    .immediate_send(scope, buf);
                reqs.add(req);
            }
            drop(send_time);

            let receive_span = span!(Level::TRACE, "receive_msgs", rank = self.rank(), now = now);
            let handle_span = span!(Level::TRACE, "handle_msgs", rank = self.rank(), now = now);
            // Use blocking MPI_recv here, since we don't have anything to do if there are no other
            // messages.
            while !expected_vehicle_messages.is_empty() {
                // measure the wait time for receiving
                let receive_time = receive_span.enter();
                let (encoded_msg, _status) = self.mpi_communicator.any_process().receive_vec();
                drop(receive_time);

                let handle_time = handle_span.enter();
                let msg = SimMessage::deserialize(&encoded_msg).sync_message();
                let from_rank = msg.from_process;

                // If a message was received from a neighbor partition for this very time step, remove
                // that partition from expected messages which indicates which partitions we are waiting
                // for
                if msg.time == now {
                    expected_vehicle_messages.remove(&from_rank);
                }

                on_msg(msg);
                drop(handle_time);
            }

            // wait here, so that all requests finish. This is necessary, because a process might send
            // more messages than it receives. This happens, if a process sends messages to remote
            // partitions (teleported legs) but only receives messages from neighbor partitions.
            // this also accounts for wait times
            let receive_time = receive_span.enter();
            reqs.wait_all(&mut Vec::new());
            drop(receive_time)
        });
    }

    fn barrier(&self) {
        self.mpi_communicator.barrier();
    }

    fn rank(&self) -> u32 {
        self.mpi_communicator.rank() as u32
    }
}

impl MpiSimCommunicator {
    pub(crate) fn new(mpi_communicator: SimpleCommunicator) -> Self {
        MpiSimCommunicator { mpi_communicator }
    }
}
