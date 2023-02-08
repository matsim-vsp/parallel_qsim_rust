use crate::simulation::config::RoutingMode;
use crate::simulation::messaging::messages::proto::{
    SimulationUpdateMessage, TrafficInfoMessage, Vehicle, VehicleMessage,
};
use crate::simulation::network::node::NodeVehicle;
use mpi::datatype::PartitionMut;
use mpi::topology::SystemCommunicator;
use mpi::traits::{Communicator, CommunicatorCollectives, Destination, Source};
use mpi::{Count, Rank};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;

pub trait MessageBroker {
    fn send_recv(&mut self, now: u32) -> Vec<SimulationUpdateMessage>;
    fn add_veh(&mut self, vehicle: Vehicle, now: u32);
    fn add_travel_times(&mut self, travel_times: HashMap<u64, u32>);
}
pub struct MpiMessageBroker {
    pub rank: Rank,
    communicator: SystemCommunicator,
    neighbors: HashSet<u32>,
    out_messages: HashMap<u32, VehicleMessage>,
    in_messages: BinaryHeap<VehicleMessage>,
    link_id_mapping: Arc<HashMap<usize, usize>>,
    traffic_info: TrafficInfoMessage,
    routing_mode: RoutingMode,
}

impl MessageBroker for MpiMessageBroker {
    fn send_recv(&mut self, now: u32) -> Vec<SimulationUpdateMessage> {
        // preparation of traffic info messages
        let traffic_info_message = self.traffic_info.serialize();

        // preparation of vehicle messages
        let vehicle_messages = self.prepare_send_recv_vehicles(now);
        let buf_msg: Vec<_> = vehicle_messages
            .values()
            .map(|m| (m, m.serialize()))
            .collect();

        // prepare the receiving here.
        let mut expected_vehicle_messages = self.neighbors.clone();
        let mut received_vehicle_messages = Vec::new();
        let mut received_traffic_info_messages = Vec::new();

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
                    .communicator
                    .process_at_rank(message.to_process as Rank)
                    .immediate_send(scope, buf);
                reqs.add(req);
            }

            // ------ Gather traffic information --------
            if self.routing_mode == RoutingMode::AdHoc && now % (60 * 15) == 0 {
                received_traffic_info_messages = self.gather_traffic_info(&traffic_info_message);
            }

            // ------ Receive Part --------
            self.pop_from_cache(
                &mut expected_vehicle_messages,
                &mut received_vehicle_messages,
                now,
            );

            // Use blocking MPI_recv here, since we don't have anything to do if there are no other
            // messages.
            while !expected_vehicle_messages.is_empty() {
                let (encoded_msg, _status) = self.communicator.any_process().receive_vec();
                let msg = VehicleMessage::deserialize(&encoded_msg);
                let from_rank = msg.from_process;

                // If a message was received from a neighbor partition for this very time step, remove
                // that partition from expected messages which indicates which partitions we are waiting
                // for
                if msg.time == now {
                    expected_vehicle_messages.remove(&from_rank);
                }
                // In case a message is for a future time step store it in the message cache, until
                // this process reaches the time step of that message. Otherwise store it in received
                // messages and use it in the simulation
                if msg.time <= now {
                    received_vehicle_messages
                        .push(SimulationUpdateMessage::new_vehicle_message(msg));
                } else {
                    self.in_messages.push(msg);
                }
            }

            // wait here, so that all requests finish. This is necessary, because a process might send
            // more messages than it receives. This happens, if a process sends messages to remote
            // partitions (teleported legs) but only receives messages from neighbor partitions.
            reqs.wait_all(&mut Vec::new());
        });

        let mut result: Vec<SimulationUpdateMessage> = Vec::new();
        result.append(&mut received_vehicle_messages);
        result.append(&mut received_traffic_info_messages);
        result
    }

    fn add_veh(&mut self, vehicle: Vehicle, now: u32) {
        let link_id = vehicle.curr_link_id().unwrap();
        let partition = *self.link_id_mapping.get(&link_id).unwrap() as u32;
        let message = self
            .out_messages
            .entry(partition)
            .or_insert_with(|| VehicleMessage::new(now, self.rank as u32, partition));
        message.add(vehicle);
    }

    fn add_travel_times(&mut self, new_travel_times: HashMap<u64, u32>) {
        self.traffic_info.travel_times = new_travel_times;
    }
}

impl MpiMessageBroker {
    pub fn new(
        communicator: SystemCommunicator,
        rank: Rank,
        neighbors: HashSet<u32>,
        link_id_mapping: Arc<HashMap<usize, usize>>,
        routing_mode: RoutingMode,
    ) -> Self {
        MpiMessageBroker {
            out_messages: HashMap::new(),
            in_messages: BinaryHeap::new(),
            communicator,
            neighbors,
            rank,
            link_id_mapping,
            traffic_info: TrafficInfoMessage::new(),
            routing_mode,
        }
    }

    pub fn rank_for_link(&self, link_id: u64) -> u64 {
        *self.link_id_mapping.get(&(link_id as usize)).unwrap() as u64
    }

    fn pop_from_cache(
        &mut self,
        expected_messages: &mut HashSet<u32>,
        messages: &mut Vec<SimulationUpdateMessage>,
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
                messages.push(SimulationUpdateMessage::new_vehicle_message(
                    self.in_messages.pop().unwrap(),
                ))
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

    fn get_traffic_info_displs(all_traffic_info_message_lengths: &mut Vec<i32>) -> Vec<Count> {
        // this is copied from rsmpi example immediate_all_gather_varcount
        all_traffic_info_message_lengths
            .iter()
            .scan(0, |acc, &x| {
                let tmp = *acc;
                *acc += x;
                Some(tmp)
            })
            .collect()
    }

    fn deserialize_traffic_infos(
        all_traffic_info_messages: Vec<u8>,
        lengths: Vec<i32>,
    ) -> Vec<SimulationUpdateMessage> {
        let mut result = Vec::new();
        let mut last_end_index = 0usize;
        for len in lengths {
            let begin_index = last_end_index;
            let end_index = last_end_index + len as usize;
            result.push(SimulationUpdateMessage::new_traffic_info_message(
                TrafficInfoMessage::deserialize(
                    &all_traffic_info_messages[begin_index..end_index as usize],
                ),
            ));
            last_end_index = end_index;
        }
        result
    }

    fn gather_traffic_info(
        &mut self,
        traffic_info_message: &Vec<u8>,
    ) -> Vec<SimulationUpdateMessage> {
        // ------- Gather traffic info lengths -------
        let mut traffic_info_length_buffer = vec![0i32; self.communicator.size() as usize];
        self.communicator.all_gather_into(
            &(traffic_info_message.len() as i32),
            &mut traffic_info_length_buffer[..],
        );

        // ------- Gather traffic info -------
        if traffic_info_length_buffer.iter().sum::<i32>() <= 0 {
            // if there is no traffic data to be sent, we do not actually perform mpi communication
            // because mpi would crash
            return Vec::new();
        }

        let mut traffic_info_buffer =
            vec![0u8; traffic_info_length_buffer.iter().sum::<i32>() as usize];
        let info_displs = Self::get_traffic_info_displs(&mut traffic_info_length_buffer);
        let mut partition = PartitionMut::new(
            &mut traffic_info_buffer,
            traffic_info_length_buffer.clone(),
            &info_displs[..],
        );
        self.communicator
            .all_gather_varcount_into(&traffic_info_message[..], &mut partition);

        Self::deserialize_traffic_infos(traffic_info_buffer, traffic_info_length_buffer)
    }
}
