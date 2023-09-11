use std::collections::HashMap;

use crate::simulation::id::{Id, IdStore};
use crate::simulation::messaging::messages::proto::Vehicle;
use crate::simulation::vehicles::VehicleType::VehicleType;

#[derive(Debug)]
pub struct Garage<'g> {
    vehicles: HashMap<Id<Vehicle>, Vehicle>,
    vehicle_ids: IdStore<'g, Vehicle>,
    vehicle_types: Vec<VehicleType>,
    vehicle_type_ids: IdStore<'g, VehicleType>,
    modes: IdStore<'g, String>,
}

impl<'g> Default for Garage<'g> {
    fn default() -> Self {
        Garage::new()
    }
}

impl<'g> Garage<'g> {
    pub fn new() -> Self {
        Garage {
            vehicles: Default::default(),
            vehicle_ids: Default::default(),
            vehicle_types: vec![],
            vehicle_type_ids: IdStore::new(),
            modes: Default::default(),
        }
    }
}
