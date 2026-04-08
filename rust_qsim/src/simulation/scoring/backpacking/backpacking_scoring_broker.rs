use crate::simulation::messaging::sim_communication::SimCommunicator;

//TODO currently, the only way of realising an independent message broker was to reference the original
// simcommunicator with a lifetime. A unified and modular solution should be discussed. aleks Apr'26
pub struct BackpackingMessageBroker<'a, C>
where
    C: SimCommunicator
{
    communicator: &'a C,
}

impl<'a, C> BackpackingMessageBroker<'a, C>
where
    C: SimCommunicator,
{
    pub fn new(communicator: &'a C) -> Self {
        Self { communicator }
    }

}
