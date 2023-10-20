use nohash_hasher::IntMap;

use crate::simulation::id::{Id, IdStore};
use crate::simulation::io::vehicles::{IOVehicleDefinitions, IOVehicleType};
use crate::simulation::messaging::messages::proto::{Agent, Vehicle};
use crate::simulation::vehicles::vehicle_type::{LevelOfDetail, VehicleType};

#[derive(Debug)]
pub struct Garage<'g> {
    pub network_vehicles: IntMap<Id<Vehicle>, GarageVehicle>,
    pub vehicle_ids: IdStore<'g, Vehicle>,
    pub teleported_veh: IntMap<Id<Vehicle>, Id<VehicleType>>,
    pub vehicle_types: IntMap<Id<VehicleType>, VehicleType>,
    pub vehicle_type_ids: IdStore<'g, VehicleType>,
}

#[derive(Debug)]
pub struct GarageVehicle {
    pub id: Id<Vehicle>,
    pub veh_type: Id<VehicleType>,
}

impl<'g> Default for Garage<'g> {
    fn default() -> Self {
        Garage::new()
    }
}

impl<'g> Garage<'g> {
    pub fn new() -> Self {
        Garage {
            network_vehicles: Default::default(),
            vehicle_ids: Default::default(),
            teleported_veh: Default::default(),
            vehicle_types: Default::default(),
            vehicle_type_ids: IdStore::new(),
        }
    }

    pub fn from_file(file_path: &str, mode_store: &mut IdStore<String>) -> Self {
        let io_veh_definition = IOVehicleDefinitions::from_file(file_path);
        let mut result = Self::new();
        for io_veh_type in io_veh_definition.veh_types {
            result.add_io_veh_type(io_veh_type, mode_store);
        }
        result
    }

    pub fn add_io_veh_type(
        &mut self,
        io_veh_type: IOVehicleType,
        mode_store: &mut IdStore<String>,
    ) {
        let id = self.vehicle_type_ids.create_id(&io_veh_type.id);
        let net_mode =
            mode_store.create_id(&io_veh_type.network_mode.unwrap_or_default().network_mode);
        let lod = if let Some(attr) = io_veh_type
            .attributes
            .unwrap_or_default()
            .attributes
            .iter()
            .find(|&attr| attr.name.eq("lod"))
        {
            LevelOfDetail::from(attr.value.as_str())
        } else {
            LevelOfDetail::from("network")
        };

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
            lod,
        };
        self.add_veh_type(veh_type);
    }

    pub fn add_veh_type(&mut self, veh_type: VehicleType) {
        assert_eq!(
            veh_type.id.internal(),
            self.vehicle_types.len() as u64,
            "internal id {} and slot in node vec {} were note the same. Probably, vehicle type {} already exists.",
            veh_type.id.internal(),
            self.vehicle_types.len(),
            veh_type.id.external()
        );

        self.vehicle_types.insert(veh_type.id.clone(), veh_type);
    }

    pub fn add_veh_id(&mut self, person_id: &Id<Agent>, type_id: &Id<VehicleType>) -> Id<Vehicle> {
        let veh_id_ext = format!("{}_{}", person_id.external(), type_id.external());
        let veh_id = self.vehicle_ids.create_id(&veh_id_ext);

        let veh_type = self.vehicle_types.get(type_id).unwrap();
        match veh_type.lod {
            LevelOfDetail::Network => {}
            LevelOfDetail::Teleported => {
                self.teleported_veh
                    .insert(veh_id.clone(), veh_type.id.clone());
            }
        };

        veh_id
    }

    pub fn add_veh(&mut self, veh_id: Id<Vehicle>, veh_type_id: Id<VehicleType>) {
        let veh_type = self.vehicle_types.get(&veh_type_id).unwrap();
        match veh_type.lod {
            LevelOfDetail::Network => {
                let vehicle = GarageVehicle {
                    id: veh_id.clone(),
                    veh_type: veh_type_id.clone(),
                };
                self.network_vehicles.insert(vehicle.id.clone(), vehicle);
            }
            LevelOfDetail::Teleported => {}
        }
    }

    pub fn get_mode_veh_id(&self, person_id: &Id<Agent>, mode: &Id<String>) -> Id<Vehicle> {
        let external = format!("{}_{}", person_id.external(), mode.external());
        self.vehicle_ids.get_from_ext(&external)
    }

    pub(crate) fn park_veh(&mut self, vehicle: Vehicle) -> Agent {
        let id = self.vehicle_ids.get(vehicle.id);
        let veh_type = self.vehicle_type_ids.get(vehicle.r#type);
        let garage_veh = GarageVehicle { id, veh_type };
        self.network_vehicles
            .insert(garage_veh.id.clone(), garage_veh);

        vehicle.agent.unwrap()
    }

    pub fn unpark_veh(&mut self, person: Agent, id: &Id<Vehicle>) -> Vehicle {
        let veh_type_id = if let Some(veh_type_id) = self.teleported_veh.get(id) {
            veh_type_id.clone()
        } else if let Some(garage_veh) = self.network_vehicles.remove(id) {
            garage_veh.veh_type
        } else {
            panic!(
                "Can't unpark vehicle with id {}. It was not parked in this garage.",
                id.external()
            );
        };

        let veh_type = self.vehicle_types.get(&veh_type_id).unwrap();

        Vehicle {
            id: id.internal(),
            curr_route_elem: 0,
            r#type: veh_type.id.internal(),
            max_v: veh_type.max_v,
            pce: veh_type.pce,
            agent: Some(person),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::{Id, IdStore};
    use crate::simulation::io::attributes::{Attr, Attrs};
    use crate::simulation::io::vehicles::{
        IODimension, IOFowEfficiencyFactor, IONetworkMode, IOPassengerCarEquivalents,
        IOVehicleType, IOVelocity,
    };
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::vehicles::vehicle_type::{LevelOfDetail, VehicleType};

    #[test]
    fn add_veh_type() {
        let mut garage = Garage::new();
        let type_id = garage.vehicle_type_ids.create_id("some-type");
        let mode = Id::new_internal(0);
        let veh_type = VehicleType::new(type_id, mode);

        garage.add_veh_type(veh_type);

        assert_eq!(1, garage.vehicle_types.len());
    }

    #[test]
    #[should_panic]
    fn add_veh_type_reject_duplicate() {
        let mut garage = Garage::new();
        let type_id = garage.vehicle_type_ids.create_id("some-type");
        let mode = Id::new_internal(0);
        let veh_type1 = VehicleType::new(type_id.clone(), mode.clone());
        let veh_type2 = VehicleType::new(type_id.clone(), mode.clone());

        garage.add_veh_type(veh_type1);
        garage.add_veh_type(veh_type2);
    }

    #[test]
    fn add_empty_io_veh_type() {
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
        let mut mode_store = IdStore::new();

        garage.add_io_veh_type(io_veh_type, &mut mode_store);

        assert_eq!(1, garage.vehicle_types.len());
        assert_eq!(0, mode_store.get_from_ext("car").internal());
        assert_eq!(
            0,
            garage.vehicle_type_ids.get_from_ext("some-id").internal()
        );

        let veh_type_opt = garage.vehicle_types.values().next();
        assert!(veh_type_opt.is_some());
        let veh_type = veh_type_opt.unwrap();
        assert!(matches!(veh_type.lod, LevelOfDetail::Network));
    }

    #[test]
    fn add_io_veh_type() {
        let io_veh_type = IOVehicleType {
            id: "some-id".to_string(),
            description: None,
            capacity: None,
            length: Some(IODimension { meter: 10. }),
            width: Some(IODimension { meter: 5. }),
            maximum_velocity: Some(IOVelocity {
                meter_per_second: 100.,
            }),
            engine_information: None,
            cost_information: None,
            passenger_car_equivalents: Some(IOPassengerCarEquivalents { pce: 21.0 }),
            network_mode: Some(IONetworkMode {
                network_mode: "some_mode".to_string(),
            }),
            flow_efficiency_factor: Some(IOFowEfficiencyFactor { factor: 2. }),
            attributes: Some(Attrs {
                attributes: vec![Attr::new(String::from("lod"), String::from("teleported"))],
            }),
        };
        let mut garage = Garage::new();
        let mut mode_store = IdStore::new();

        garage.add_io_veh_type(io_veh_type, &mut mode_store);

        let expected_id = garage.vehicle_type_ids.get_from_ext("some-id");
        let expected_mode = mode_store.get_from_ext("some_mode");

        let veh_type_opt = garage.vehicle_types.values().next();
        assert!(veh_type_opt.is_some());
        let veh_type = veh_type_opt.unwrap();
        assert!(matches!(veh_type.lod, LevelOfDetail::Teleported));
        assert_eq!(veh_type.max_v, 100.);
        assert_eq!(veh_type.width, 5.0);
        assert_eq!(veh_type.length, 10.);
        assert_eq!(veh_type.pce, 21.);
        assert_eq!(veh_type.fef, 2.);
        assert_eq!(veh_type.id, expected_id);
        assert_eq!(veh_type.net_mode, expected_mode)
    }

    #[test]
    fn from_file() {
        let mut mode_store = IdStore::new();
        let garage = Garage::from_file("./assets/3-links/vehicles.xml", &mut mode_store);
        assert_eq!(3, garage.vehicle_types.len());
    }
}
