use std::path::Path;

use crate::simulation::id::Id;
use crate::simulation::io::xml::vehicles::{IOVehicle, IOVehicleDefinitions, IOVehicleType};
use crate::simulation::population::InternalPerson;
use crate::simulation::vehicles::{from_file, to_file, InternalVehicle, InternalVehicleType};
use crate::simulation::SimulationAgent;
use nohash_hasher::IntMap;
use tracing::info;

//TODO rename to Vehicles
pub struct Garage {
    pub vehicles: IntMap<Id<InternalVehicle>, InternalVehicle>,
    pub vehicle_types: IntMap<Id<InternalVehicleType>, InternalVehicleType>,
}

impl Default for Garage {
    fn default() -> Self {
        Garage::new()
    }
}

impl From<IOVehicleDefinitions> for Garage {
    fn from(io_vehicles: IOVehicleDefinitions) -> Self {
        let mut result = Garage::new();
        for io_veh_type in io_vehicles.veh_types {
            add_io_veh_type(&mut result, io_veh_type);
        }
        for io_veh in io_vehicles.vehicles {
            add_io_veh(&mut result, io_veh)
        }
        let keys_ext: Vec<_> = result.vehicle_types.keys().map(|k| k.external()).collect();
        info!(
            "Created Garage from file with vehicle types: {:?}",
            keys_ext
        );
        result
    }
}

fn add_io_veh_type(garage: &mut Garage, io_veh_type: IOVehicleType) {
    let id: Id<InternalVehicleType> = Id::create(&io_veh_type.id);
    let net_mode: Id<String> =
        Id::create(&io_veh_type.network_mode.unwrap_or_default().network_mode);

    let veh_type = InternalVehicleType {
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
        attributes: io_veh_type.attributes.map(Into::into).unwrap_or_default(),
    };
    garage.add_veh_type(veh_type);
}

fn add_io_veh(garage: &mut Garage, io_veh: IOVehicle) {
    let veh_type = garage.vehicle_types.get(&Id::get_from_ext(io_veh.vehicle_type.as_str()))
        .expect("Vehicle type of vehicle not found. There has to be a vehicle type defined before a vehicle can be added.");
    let vehicle = InternalVehicle::from_io(io_veh, veh_type);

    //add id for drt mode
    if let Some(o) = vehicle.attributes.get::<String>("dvrpMode") {
        Id::<String>::create(o.as_str());
    }

    garage.add_veh(vehicle);
}

impl Garage {
    pub fn new() -> Self {
        Garage {
            vehicles: Default::default(),
            vehicle_types: Default::default(),
        }
    }

    pub fn from_file(file_path: &Path) -> Self {
        from_file(file_path)
    }

    pub fn to_file(&self, file_path: &Path) {
        to_file(self, file_path);
    }

    pub fn add_veh_type(&mut self, veh_type: InternalVehicleType) {
        assert!(
            !self.vehicle_types.contains_key(&veh_type.id),
            "Vehicle type with id {:?} already exists.",
            &veh_type.id
        );

        self.vehicle_types.insert(veh_type.id.clone(), veh_type);
    }

    pub fn add_veh_by_type(
        &mut self,
        person_id: &Id<InternalPerson>,
        type_id: &Id<InternalVehicleType>,
    ) {
        let veh_id_ext = format!("{}_{}", person_id.external(), type_id.external());
        let veh_id = Id::create(&veh_id_ext);

        let veh_type = self.vehicle_types.get(type_id).unwrap();

        let vehicle = InternalVehicle {
            id: veh_id,
            driver: None,
            passengers: vec![],
            vehicle_type: veh_type.id.clone(),
            attributes: Default::default(),
            max_v: veh_type.max_v,
            pce: veh_type.pce,
        };

        self.add_veh(vehicle);
    }

    pub fn add_veh(&mut self, veh: InternalVehicle) {
        let id = veh.id.clone();
        self.vehicles.insert(id, veh);
    }

    pub fn veh_id(
        &self,
        person_id: &Id<InternalPerson>,
        veh_type_id: &Id<InternalVehicleType>,
    ) -> Id<InternalVehicle> {
        let external = format!("{}_{}", person_id.external(), veh_type_id.external());
        Id::get_from_ext(&external)
    }

    pub(crate) fn park_veh(&mut self, mut vehicle: InternalVehicle) -> Vec<SimulationAgent> {
        let mut agents = std::mem::take(&mut vehicle.passengers);
        let person = vehicle.driver.take().expect("Vehicle has no driver.");
        agents.push(person);
        agents

        // This would be need for mass conservation, but is not implemented yet.
        // Thus, we just take driver and passengers and forget about the vehicle itself.

        // self.vehicles.insert(Id::get(vehicle.id), vehicle);
    }

    pub fn unpark_veh_with_passengers(
        &mut self,
        agent: SimulationAgent,
        passengers: Vec<SimulationAgent>,
        id: Id<InternalVehicle>,
    ) -> InternalVehicle {
        let veh_type_id = &self
            .vehicles
            .get(&id)
            .unwrap_or_else(|| panic!("Can't unpark vehicle with id {id}. It was not parked in this garage. Vehicle: {:?}", self.vehicles.len())).vehicle_type;

        let veh_type = self.vehicle_types.get(veh_type_id).unwrap();

        InternalVehicle {
            id: id.clone(),
            max_v: veh_type.max_v,
            pce: veh_type.pce,
            driver: Some(agent),
            passengers,
            vehicle_type: veh_type_id.clone(),
            attributes: Default::default(),
        }

        // The following code would be used if mass conservation is enabled. But, there are some pitfalls.
        // One would need to configure for which vehicle types this is allowed.
        // Currently (apr '25), each and every mode needs to be a vehicle type, in particular walking.
        // But, this makes mass conservation complicated. Imagine a person walking from partition 1 -> 2, driving car from 2 -> 3 and then walk from 3 -> 1.
        // The "walk" vehicle would be parked at partition 2, but partition 3 would need it. Consequently, the simulation crashes right now.

        // let vehicle = self.vehicles.remove(&id).unwrap();
        // let mut vehicle = vehicle;
        // vehicle.driver = Some(person);
        // vehicle.passengers = passengers;
        // vehicle
    }

    pub fn unpark_veh(
        &mut self,
        agent: SimulationAgent,
        id: Id<InternalVehicle>,
    ) -> InternalVehicle {
        self.unpark_veh_with_passengers(agent, vec![], id)
    }

    pub fn vehicle_type_id(&self, veh: &Id<InternalVehicle>) -> Id<InternalVehicleType> {
        self.vehicles
            .get(veh)
            .map(|v| v.vehicle_type.clone())
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::simulation::id::Id;
    use crate::simulation::io::xml::attributes::{IOAttribute, IOAttributes};
    use crate::simulation::io::xml::vehicles::{
        IODimension, IOFowEfficiencyFactor, IONetworkMode, IOPassengerCarEquivalents,
        IOVehicleType, IOVelocity,
    };
    use crate::simulation::vehicles::garage::{add_io_veh_type, Garage};
    use crate::simulation::vehicles::{InternalVehicle, InternalVehicleType};
    use crate::test_utils::create_vehicle_type;

    #[test]
    fn add_veh_type() {
        let mut garage = Garage::new();
        let type_id = Id::create("some-type");
        let mode = Id::new_internal(0);
        let veh_type = create_vehicle_type(&type_id, mode);

        garage.add_veh_type(veh_type);

        assert_eq!(1, garage.vehicle_types.len());
    }

    #[test]
    #[should_panic]
    fn add_veh_type_reject_duplicate() {
        let mut garage = Garage::new();
        let type_id = Id::create("some-type");
        let mode = Id::new_internal(0);
        let veh_type1 = create_vehicle_type(&type_id, mode.clone());
        let veh_type2 = create_vehicle_type(&type_id, mode.clone());

        garage.add_veh_type(veh_type1);
        garage.add_veh_type(veh_type2);
    }

    #[test]
    fn add_vehicle_without_type() {
        let mut garage = Garage::new();
        garage.add_veh(InternalVehicle {
            id: Id::create("0"),
            max_v: 0.0,
            pce: 0.0,
            driver: None,
            passengers: vec![],
            vehicle_type: Id::create("0"),
            attributes: Default::default(),
        });
    }

    #[test]
    fn add_vehicle() {
        // prepare garage with type
        let mut garage = Garage::new();
        let type_id = Id::create("vehicle_type");
        let mode = Id::create("car");
        let veh_type = create_vehicle_type(&type_id, mode);
        let veh_type_id = veh_type.id.clone();
        garage.add_veh_type(veh_type);

        let id = Id::<InternalVehicle>::create("veh");
        garage.add_veh(InternalVehicle {
            id,
            max_v: 0.0,
            pce: 0.0,
            driver: None,
            passengers: vec![],
            vehicle_type: veh_type_id,
            attributes: Default::default(),
        });

        assert_eq!(1, garage.vehicles.len());
    }

    #[test]
    fn from_file() {
        let garage = Garage::from_file(&PathBuf::from("./assets/3-links/vehicles.xml"));
        assert_eq!(3, garage.vehicle_types.len());
        assert_eq!(0, garage.vehicles.len());
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

        add_io_veh_type(&mut garage, io_veh_type);

        assert_eq!(1, garage.vehicle_types.len());

        // Check if IDs are created correctly
        Id::<String>::get_from_ext("car");
        Id::<InternalVehicleType>::get_from_ext("some-id");

        let veh_type_opt = garage.vehicle_types.values().next();
        assert!(veh_type_opt.is_some());
    }

    #[test]
    fn test_add_io_veh_type() {
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
            attributes: Some(IOAttributes {
                attributes: vec![IOAttribute::new(
                    String::from("lod"),
                    String::from("teleported"),
                )],
            }),
        };
        let mut garage = Garage::new();
        add_io_veh_type(&mut garage, io_veh_type);

        let expected_id: Id<InternalVehicleType> = Id::get_from_ext("some-id");
        let expected_mode: Id<String> = Id::get_from_ext("some_mode");

        let veh_type_opt = garage.vehicle_types.values().next();
        assert!(veh_type_opt.is_some());
        let veh_type = veh_type_opt.unwrap();
        assert_eq!(veh_type.max_v, 100.);
        assert_eq!(veh_type.width, 5.0);
        assert_eq!(veh_type.length, 10.);
        assert_eq!(veh_type.pce, 21.);
        assert_eq!(veh_type.fef, 2.);
        assert_eq!(veh_type.id.internal(), expected_id.internal());
        assert_eq!(veh_type.net_mode.internal(), expected_mode.internal())
    }
}
