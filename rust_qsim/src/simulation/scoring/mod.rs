use std::collections::{HashMap, HashSet};
use crate::simulation::events::EventHandlerRegisterFn;

mod backpacking;

pub trait ScoringEngine {
    fn scoring();
}

/// A data collector registers callbacks for events, that are important for the respective ScoringEngine
pub trait DataCollector {
    fn register_fn() -> Box<EventHandlerRegisterFn>;
}

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

// TODO These structs are yet to be implemented in respective sub-modules
pub struct PlanCollectingMessageBroker {

}

pub struct IntegratedPlanCollectingMessageBroker {

}

pub struct OutsourcedPlanCollectingMessageBroker {

}
