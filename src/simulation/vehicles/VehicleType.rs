use crate::simulation::id::Id;

#[derive(Debug)]
pub struct VehicleType {
    id: Id<VehicleType>,
    length: f32,
    width: f32,
    max_v: f32,
    pce: f32,
    fef: f32,
    net_mode: Id<String>,
}

impl VehicleType {
    pub fn new(id: Id<VehicleType>, net_mode: Id<String>) -> Self {
        VehicleType {
            id,
            length: 0.0,
            width: 0.0,
            max_v: 0.0,
            pce: 0.0,
            fef: 0.0,
            net_mode,
        }
    }
}
