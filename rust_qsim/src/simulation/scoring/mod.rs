use std::any::{Any, TypeId};
use crate::simulation::messaging::sim_communication::SimCommunicator;

pub mod backpacking;

/// A scoring engine contains a DataCollector and MessageBroker for respective implementation.
pub trait ScoringEngine
{
    fn create_for_n_partitions(n: u32);
    fn scoring(&self);
}

// A data collector registers callbacks for events, that are important for the respective ScoringEngine
// pub trait DataCollector {
//     fn register_fn(data_collector: Arc<Mutex<dyn DataCollector>>) -> Box<EventHandlerRegisterFn>;
// }

pub trait Message: Any + Send {
    fn as_any(&self) -> &dyn Any;

    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: Any + Send> Message for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub struct InternalScoringMessage {
    // time: u32,
    from_process: u32,
    to_process: u32,
    message: Box<dyn Message>
}

/// The message broker communicates with other partitions
pub trait ScoringMessageBroker{

}

// TODO These structs are yet to be implemented in respective sub-modules
pub struct PlanCollectingMessageBroker {

}

pub struct IntegratedPlanCollectingMessageBroker {

}

pub struct OutsourcedPlanCollectingMessageBroker {

}
