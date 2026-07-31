use crate::simulation::Identifiable;
use crate::simulation::agents::EndTime;
use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::id::Id;
use crate::simulation::network::sim_network::StorageUpdate;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::time::{SimTime, Tick};
use crate::simulation::vehicles::SimulationVehicle;
use std::cmp::Ordering;

pub enum InternalSimMessage {
    Sync(InternalSyncMessage),
    Barrier,
}

#[derive(Debug)]
pub(crate) struct ScheduledTeleportation {
    agent: SimulationAgent,
    end_time: SimTime,
}

impl ScheduledTeleportation {
    pub(crate) fn new(agent: SimulationAgent, end_time: SimTime) -> Self {
        Self { agent, end_time }
    }

    pub(crate) fn agent(&self) -> &SimulationAgent {
        &self.agent
    }

    pub(crate) fn end_time(&self) -> SimTime {
        self.end_time
    }

    pub(crate) fn into_agent(self) -> SimulationAgent {
        self.agent
    }
}

impl EndTime for ScheduledTeleportation {
    fn end_time(&self, _now: SimTime) -> SimTime {
        self.end_time
    }
}

impl Identifiable<InternalPerson> for ScheduledTeleportation {
    fn id(&self) -> &Id<InternalPerson> {
        self.agent.id()
    }
}

#[derive(Debug)]
pub struct InternalSyncMessage {
    time: Tick,
    from_process: u32,
    to_process: u32,
    vehicles: Vec<SimulationVehicle>,
    teleportations: Vec<ScheduledTeleportation>,
    storage_capacities: Vec<StorageUpdate>,
}

impl InternalSimMessage {
    pub fn sync_message(self) -> InternalSyncMessage {
        match self {
            InternalSimMessage::Sync(m) => m,
            _ => panic!("That message is no sync message."),
        }
    }

    pub fn from_sync_message(m: InternalSyncMessage) -> InternalSimMessage {
        InternalSimMessage::Sync(m)
    }

    pub fn barrier() -> InternalSimMessage {
        InternalSimMessage::Barrier
    }
}

impl InternalSyncMessage {
    pub fn new(time: Tick, from: u32, to: u32) -> Self {
        Self {
            time,
            from_process: from,
            to_process: to,
            vehicles: Vec::new(),
            teleportations: Vec::new(),
            storage_capacities: Vec::new(),
        }
    }

    pub fn add_veh(&mut self, vehicle: SimulationVehicle) {
        self.vehicles.push(vehicle);
    }

    pub(crate) fn add_teleportation(&mut self, teleportation: ScheduledTeleportation) {
        self.teleportations.push(teleportation);
    }

    pub fn add_storage_cap(&mut self, storage_cap: StorageUpdate) {
        self.storage_capacities.push(storage_cap);
    }

    pub fn time(&self) -> Tick {
        self.time
    }

    pub fn from_process(&self) -> u32 {
        self.from_process
    }

    pub fn to_process(&self) -> u32 {
        self.to_process
    }

    pub fn vehicles(&self) -> &Vec<SimulationVehicle> {
        &self.vehicles
    }

    pub fn vehicles_mut(&mut self) -> &mut Vec<SimulationVehicle> {
        &mut self.vehicles
    }

    #[cfg(test)]
    pub(crate) fn teleportations(&self) -> &[ScheduledTeleportation] {
        &self.teleportations
    }

    pub fn storage_capacities(&self) -> &Vec<StorageUpdate> {
        &self.storage_capacities
    }

    pub fn take_storage_capacities(&mut self) -> Vec<StorageUpdate> {
        std::mem::take(&mut self.storage_capacities)
    }

    pub fn take_vehicles(&mut self) -> Vec<SimulationVehicle> {
        std::mem::take(&mut self.vehicles)
    }

    pub(crate) fn take_teleportations(&mut self) -> Vec<ScheduledTeleportation> {
        std::mem::take(&mut self.teleportations)
    }
}

impl PartialEq for InternalSyncMessage {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

// Implementation for ordering, so that vehicle messages can be put into a message queue sorted by time
impl PartialOrd for InternalSyncMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for InternalSyncMessage {}

impl Ord for InternalSyncMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.cmp(&self.time)
    }
}

#[cfg(test)]
mod tests {
    use super::{InternalSyncMessage, ScheduledTeleportation};
    use crate::simulation::Identifiable;
    use crate::simulation::time::{SimTime, Tick};
    use crate::test_utils::create_agent;
    use macros::deterministic_id_test;

    #[deterministic_id_test]
    fn sync_message_preserves_scheduled_teleportation() {
        let agent = create_agent(7, vec!["destination"]);
        let end_time = SimTime::from_nanos(12_345_678_900);
        let mut message = InternalSyncMessage::new(Tick::new(2), 1, 3);

        message.add_teleportation(ScheduledTeleportation::new(agent, end_time));

        assert_eq!(message.teleportations().len(), 1);
        assert_eq!(message.teleportations()[0].id().external(), "7");
        assert_eq!(message.teleportations()[0].end_time(), end_time);

        let teleportation = message.take_teleportations().pop().unwrap();
        assert_eq!(teleportation.id().external(), "7");
        assert_eq!(teleportation.end_time(), end_time);
        assert!(message.teleportations().is_empty());
    }
}
