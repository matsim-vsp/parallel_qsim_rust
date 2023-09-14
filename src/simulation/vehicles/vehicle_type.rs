use crate::simulation::id::Id;

#[derive(Debug)]
pub struct VehicleType {
    pub(crate) id: Id<VehicleType>,
    pub(crate) length: f32,
    pub(crate) width: f32,
    pub(crate) max_v: f32,
    pub(crate) pce: f32,
    pub(crate) fef: f32,
    pub(crate) net_mode: Id<String>,
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
