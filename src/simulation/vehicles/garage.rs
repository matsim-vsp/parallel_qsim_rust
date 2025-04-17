use std::path::Path;

use nohash_hasher::IntMap;

use crate::simulation::id::Id;
use crate::simulation::vehicles::io::{from_file, to_file};
use crate::simulation::wire_types::messages::{SimulationAgent, Vehicle};
use crate::simulation::wire_types::population::Person;
use crate::simulation::wire_types::vehicles::VehicleType;

#[derive(Debug)]
//TODO rename to Vehicles
pub struct Garage {
    pub vehicles: IntMap<Id<Vehicle>, Vehicle>,
    pub vehicle_types: IntMap<Id<VehicleType>, VehicleType>,
}

impl Default for Garage {
    fn default() -> Self {
        Garage::new()
    }
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

    pub fn add_veh_type(&mut self, veh_type: VehicleType) {
        assert!(
            !self.vehicle_types.contains_key(&Id::get(veh_type.id)),
            "Vehicle type with id {:?} already exists.",
            Id::<VehicleType>::get(veh_type.id)
        );

        self.vehicle_types.insert(Id::get(veh_type.id), veh_type);
    }

    pub fn add_veh_by_type(
        &mut self,
        person_id: &Id<Person>,
        type_id: &Id<VehicleType>,
    ) -> Id<Vehicle> {
        let veh_id_ext = format!("{}_{}", person_id.external(), type_id.external());
        let veh_id = Id::create(&veh_id_ext);

        let veh_type = self.vehicle_types.get(type_id).unwrap();

        let vehicle = Vehicle {
            id: veh_id.internal(),
            curr_route_elem: 0,
            r#type: veh_type.id,
            max_v: veh_type.max_v,
            pce: veh_type.pce,
            driver: None,
            passengers: vec![],
            attributes: Default::default(),
        };

        self.add_veh(vehicle);

        veh_id
    }

    pub fn add_veh(&mut self, veh: Vehicle) {
        let id = Id::<Vehicle>::get(veh.id);
        self.vehicles.insert(id, veh);
    }

    pub fn veh_id(&self, person_id: &Id<Person>, veh_type_id: &Id<VehicleType>) -> Id<Vehicle> {
        let external = format!("{}_{}", person_id.external(), veh_type_id.external());
        Id::get_from_ext(&external)
    }

    pub(crate) fn park_veh(&mut self, mut vehicle: Vehicle) -> Vec<SimulationAgent> {
        let mut agents = std::mem::replace(&mut vehicle.passengers, Vec::new());
        let person = std::mem::replace(&mut vehicle.driver, None).expect("Vehicle has no driver.");
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
        id: &Id<Vehicle>,
    ) -> Vehicle {
        let veh_type_id = Id::get(self
            .vehicles
            .get(id)
            .unwrap_or_else(|| panic!("Can't unpark vehicle with id {id}. It was not parked in this garage. Vehicle: {:?}", self.vehicles.len())).r#type);

        let veh_type = self.vehicle_types.get(&veh_type_id).unwrap();

        Vehicle {
            id: id.internal(),
            curr_route_elem: 0,
            r#type: veh_type.id,
            max_v: veh_type.max_v,
            pce: veh_type.pce,
            driver: Some(agent),
            passengers,
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

    pub fn unpark_veh(&mut self, agent: SimulationAgent, id: &Id<Vehicle>) -> Vehicle {
        self.unpark_veh_with_passengers(agent, vec![], id)
    }

    pub fn vehicle_type_id(&self, veh: &Id<Vehicle>) -> Id<VehicleType> {
        self.vehicles.get(veh).map(|v| Id::get(v.r#type)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::simulation::id::Id;
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::wire_types::messages::Vehicle;
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
    #[should_panic]
    fn add_vehicle_without_type() {
        let mut garage = Garage::new();
        garage.add_veh(Vehicle {
            id: 0,
            curr_route_elem: 0,
            r#type: 0,
            max_v: 0.0,
            pce: 0.0,
            driver: None,
            passengers: vec![],
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
        garage.add_veh_type(veh_type);

        let id = Id::<Vehicle>::create("veh");
        garage.add_veh(Vehicle {
            id: id.internal(),
            curr_route_elem: 0,
            r#type: type_id.internal(),
            max_v: 0.0,
            pce: 0.0,
            driver: None,
            passengers: vec![],
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
}
