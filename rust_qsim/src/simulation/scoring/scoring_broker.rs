use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use crate::simulation::scoring::backpack::Backpack;

pub struct InternalScoringMessage<T> {
    time: u32,
    from_process: u32,
    to_process: u32,
    message: T
}

pub trait ScoringMessageBroker{
    type MessageType;
    fn send_receive_scoring<F>(
        &self,
        messages: HashMap<u32, InternalScoringMessage<Self::MessageType>>,
        expected_scoring_messages: &mut HashSet<u32>,
        now: u32,
        on_msg: F,
    ) where
        F: FnMut(InternalScoringMessage<Self::MessageType>);
}

pub struct BackpackingMessageBroker{
    receiver: Receiver<InternalScoringMessage<Backpack>>,
    senders: Vec<Sender<InternalScoringMessage<Backpack>>>,
}

impl ScoringMessageBroker for BackpackingMessageBroker{
    type MessageType = Backpack;

    fn send_receive_scoring<F>(
        &self,
        messages: HashMap<u32, InternalScoringMessage<Backpack>>,
        expected_scoring_messages: &mut HashSet<u32>,
        now: u32,
        mut on_msg: F,
    ) where
        F: FnMut(InternalScoringMessage<Backpack>)
    {
        // Send messages
        for (target, msg) in messages {
            let sender = self.senders.get(target as usize).unwrap();
            sender
                .send(msg)
                .unwrap_or_else(|e| {
                panic!(
                    "Error while sending scoring message to rank {} with error {}",
                    target, e
                )
            });
        }

        // Receive messages
        while !expected_scoring_messages.is_empty() {
            let received_msg = self
                .receiver
                .recv()
                .expect("Error while receiving messages");
            let from_rank = received_msg.from_process;

            // If a message was received from a neighbor partition for this very time step, remove
            // that partition from expected messages which indicates which partitions we are waiting
            // for
            if received_msg.time == now {
                expected_scoring_messages.remove(&from_rank);
            }

            // publish the received message to the message broker
            on_msg(received_msg);
        }
    }
}

// TODO These structs are yet to be implemented
pub struct PlanCollectingMessageBroker {

}

pub struct IntegratedPlanCollectingMessageBroker {

}

pub struct OutsourcedPlanCollectingMessageBroker {

}
