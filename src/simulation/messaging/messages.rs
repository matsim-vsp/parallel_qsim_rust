use std::cmp::Ordering;

use crate::simulation::network::sim_network::StorageUpdate;
use crate::simulation::vehicles::InternalVehicle;

pub enum InternalSimMessage {
    Sync(InternalSyncMessage),
    Barrier,
}

#[derive(Debug)]
pub struct InternalSyncMessage {
    time: u32,
    from_process: u32,
    to_process: u32,
    vehicles: Vec<InternalVehicle>,
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
    pub fn new(time: u32, from: u32, to: u32) -> Self {
        Self {
            time,
            from_process: from,
            to_process: to,
            vehicles: Vec::new(),
            storage_capacities: Vec::new(),
        }
    }

    pub fn add_veh(&mut self, vehicle: InternalVehicle) {
        self.vehicles.push(vehicle);
    }

    pub fn add_storage_cap(&mut self, storage_cap: StorageUpdate) {
        self.storage_capacities.push(storage_cap);
    }

    pub fn time(&self) -> u32 {
        self.time
    }

    pub fn from_process(&self) -> u32 {
        self.from_process
    }

    pub fn to_process(&self) -> u32 {
        self.to_process
    }

    pub fn vehicles(&self) -> &Vec<InternalVehicle> {
        &self.vehicles
    }

    pub fn vehicles_mut(&mut self) -> &mut Vec<InternalVehicle> {
        &mut self.vehicles
    }

    pub fn storage_capacities(&self) -> &Vec<StorageUpdate> {
        &self.storage_capacities
    }

    pub fn take_storage_capacities(&mut self) -> Vec<StorageUpdate> {
        std::mem::take(&mut self.storage_capacities)
    }

    pub fn take_vehicles(&mut self) -> Vec<InternalVehicle> {
        std::mem::take(&mut self.vehicles)
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
