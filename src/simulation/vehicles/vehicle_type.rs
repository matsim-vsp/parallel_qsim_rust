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
    pub(crate) lod: LevelOfDetail,
}

#[derive(Debug)]
pub enum LevelOfDetail {
    Network,
    Teleported,
}

impl From<&str> for LevelOfDetail {
    fn from(value: &str) -> Self {
        match value {
            "network" => LevelOfDetail::Network,
            "teleported" => LevelOfDetail::Teleported,
            _ => panic!("&{value} is not yet implemented as level of detail"),
        }
    }
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
            lod: LevelOfDetail::Network,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::vehicles::vehicle_type::LevelOfDetail;

    #[test]
    fn lod_from_network() {
        let value = "network";
        let veh_type = LevelOfDetail::from(value);
        assert!(matches!(veh_type, LevelOfDetail::Network));
    }

    #[test]
    fn lod_from_teleported() {
        let value = "teleported";
        let veh_type = LevelOfDetail::from(value);
        assert!(matches!(veh_type, LevelOfDetail::Teleported));
    }

    #[test]
    #[should_panic]
    fn lod_from_not_supported() {
        let value = "something-invalid";
        let _ = LevelOfDetail::from(value);
    }
}
