use crate::simulation::messaging::messages::proto::{SimulationUpdateMessage, TrafficInfoMessage};
use log::debug;
use mpi::collective::CommunicatorCollectives;
use mpi::datatype::PartitionMut;
use mpi::topology::{Communicator, SystemCommunicator};
use mpi::{Count, Rank};
use std::collections::HashMap;

pub struct TrafficMessageBroker {
    pub rank: Rank,
    communicator: SystemCommunicator,
}

impl TrafficMessageBroker {
    pub fn new(communicator: SystemCommunicator, rank: Rank) -> Self {
        TrafficMessageBroker { rank, communicator }
    }

    pub fn send_recv(
        &mut self,
        now: u32,
        traffic_info: HashMap<u64, u32>,
    ) -> Vec<SimulationUpdateMessage> {
        debug!("Process {}: Traffic update at {}", self.rank, now);

        let traffic_info_message = TrafficInfoMessage::create_with(traffic_info);
        let serial_traffic_info_message = traffic_info_message.serialize();

        self.gather_traffic_info(&serial_traffic_info_message)
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
}
