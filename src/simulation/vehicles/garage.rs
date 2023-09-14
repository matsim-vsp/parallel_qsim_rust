use std::collections::HashMap;

use crate::simulation::id::{Id, IdStore};
use crate::simulation::io::vehicles::{IOVehicleDefinitions, IOVehicleType};
use crate::simulation::messaging::messages::proto::Vehicle;
use crate::simulation::vehicles::vehicle_type::VehicleType;

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

    pub fn from_file(file_path: &str) -> Self {
        let io_veh_definition = IOVehicleDefinitions::from_file(file_path);
        let mut result = Self::new();
        for io_veh_type in io_veh_definition.veh_types {
            result.add_io_veh_type(io_veh_type);
        }
        result
    }

    pub fn add_io_veh_type(&mut self, io_veh_type: IOVehicleType) {
        let id = self.vehicle_type_ids.create_id(&io_veh_type.id);
        let net_mode = self
            .modes
            .create_id(&io_veh_type.network_mode.unwrap_or_default().network_mode);

        let veh_type = VehicleType {
            id,
            length: io_veh_type.length.unwrap_or_default().meter,
            width: io_veh_type.width.unwrap_or_default().meter,
            max_v: io_veh_type
                .maximum_velocity
                .unwrap_or_default()
                .meter_per_second,
            pce: io_veh_type
                .passenger_car_equivalents
                .unwrap_or_default()
                .pce,
            fef: io_veh_type
                .flow_efficiency_factor
                .unwrap_or_default()
                .factor,
            net_mode,
        };
        self.add_veh_type(veh_type);
    }

    pub fn add_veh_type(&mut self, veh_type: VehicleType) {
        assert_eq!(
            veh_type.id.internal,
            self.vehicle_types.len(),
            "internal id {} and slot in node vec {} were note the same. Probably, node id {} already exsists.",
            veh_type.id.internal,
            self.vehicle_types.len(),
            veh_type.id.external
        );

        self.vehicle_types.push(veh_type);
    }

    pub fn add_veh_id(&mut self, external_id: &str) -> Id<Vehicle> {
        self.vehicle_ids.create_id(external_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::io::vehicles::IOVehicleType;
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::vehicles::vehicle_type::VehicleType;

    #[test]
    fn add_veh_type() {
        let mut garage = Garage::new();
        let type_id = garage.vehicle_type_ids.create_id("some-type");
        let mode = garage.modes.create_id("default-mode");
        let veh_type = VehicleType::new(type_id, mode);

        garage.add_veh_type(veh_type);

        assert_eq!(1, garage.vehicle_types.len());
    }

    #[test]
    #[should_panic]
    fn add_veh_type_reject_duplicate() {
        let mut garage = Garage::new();
        let type_id = garage.vehicle_type_ids.create_id("some-type");
        let mode = garage.modes.create_id("default-mode");
        let veh_type1 = VehicleType::new(type_id.clone(), mode.clone());
        let veh_type2 = VehicleType::new(type_id.clone(), mode.clone());

        garage.add_veh_type(veh_type1);
        garage.add_veh_type(veh_type2);
    }

    #[test]
    fn add_io_veh_type() {
        let io_veh_type = IOVehicleType {
            id: "some-id".to_string(),
            description: None,
            capacity: None,
            length: None,
            width: None,
            maximum_velocity: None,
            engine_information: None,
            cost_information: None,
            passenger_car_equivalents: None,
            network_mode: None,
            flow_efficiency_factor: None,
            attributes: None,
        };
        let mut garage = Garage::new();

        garage.add_io_veh_type(io_veh_type);

        assert_eq!(1, garage.vehicle_types.len());
        assert_eq!(0, garage.modes.get_from_ext("car").internal);
        assert_eq!(0, garage.vehicle_type_ids.get_from_ext("some-id").internal);

        assert!(garage.vehicle_types.get(0).is_some());
    }

    #[test]
    fn from_file() {
        let garage = Garage::from_file("./assets/vehicles/vehicles_v2.xml");
        assert_eq!(3, garage.vehicle_types.len());
    }
}
