use std::any::{Any};
use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{MobsimListenerRegisterFn, QSimId};

pub mod backpacking;
pub mod partial_plans;

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
    from_process: QSimId,
    #[allow(unused)]
    to_process: QSimId,
    message: Box<dyn Message>
}
