use crate::simulation::messaging::sim_communication::SimCommunicator;

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
