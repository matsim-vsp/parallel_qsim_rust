use crate::simulation::messaging::sim_communication::SimCommunicator;

pub mod backpacking;

/// A scoring engine contains a DataCollector and MessageBroker for respective implementation.
pub trait ScoringEngine<C>
where
    C: SimCommunicator,
{
    fn scoring(&self);
}

// A data collector registers callbacks for events, that are important for the respective ScoringEngine
// pub trait DataCollector {
//     fn register_fn(data_collector: Arc<Mutex<dyn DataCollector>>) -> Box<EventHandlerRegisterFn>;
// }

pub trait Message {
    fn get_message(&self) -> &dyn Message;
}

pub struct InternalScoringMessage {
    time: u32,
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
