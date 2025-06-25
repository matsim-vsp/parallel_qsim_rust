use crate::simulation::id::Id;
use crate::simulation::network::global_network::Link;
use crate::simulation::time_queue::EndTime;
use crate::simulation::vehicles::io::{IOVehicle, IOVehicleDefinitions, IOVehicleType};
use crate::simulation::wire_types::messages::SimulationAgent;
use crate::simulation::InternalAttributes;
use itertools::Itertools;

pub mod garage;
pub mod io;

#[derive(Debug, PartialEq)]
pub struct InternalVehicleType {
    pub id: Id<InternalVehicleType>,
    pub length: f32,
    pub width: f32,
    pub max_v: f32,
    pub pce: f32,
    pub fef: f32,
    pub net_mode: Id<String>,
    pub attributes: Option<InternalAttributes>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalVehicle {
    pub id: Id<InternalVehicle>,
    pub max_v: f32,
    pub pce: f32,
    pub driver: Option<SimulationAgent>,
    pub passengers: Vec<SimulationAgent>,
    pub vehicle_type: Id<InternalVehicleType>,
    pub attributes: Option<InternalAttributes>,
}

#[derive(Debug, PartialEq)]
pub struct InternalGarage {
    pub vehicle_types: Vec<InternalVehicleType>,
    pub vehicles: Vec<InternalVehicle>,
}

impl From<IOVehicleType> for InternalVehicleType {
    fn from(io: IOVehicleType) -> Self {
        InternalVehicleType {
            id: Id::create(&io.id),
            length: io.length.unwrap_or_default().meter,
            width: io.width.unwrap_or_default().meter,
            max_v: io.maximum_velocity.unwrap_or_default().meter_per_second,
            pce: io.passenger_car_equivalents.unwrap_or_default().pce,
            fef: io.flow_efficiency_factor.unwrap_or_default().factor,
            net_mode: Id::create(&io.network_mode.unwrap_or_default().network_mode),
            attributes: io.attributes.map(Into::into),
        }
    }
}

impl InternalVehicle {
    pub fn from_io(io_veh_types: &Vec<IOVehicleType>, io: IOVehicle) -> Self {
        let io_veh_type = io_veh_types
            .iter()
            .find_or_first(|io_type| io_type.id == io.id)
            .unwrap();

        InternalVehicle {
            id: Id::create(&io.id),
            max_v: io_veh_type
                .maximum_velocity
                .unwrap_or_default()
                .meter_per_second,
            pce: io_veh_type
                .passenger_car_equivalents
                .unwrap_or_default()
                .pce,
            driver: None,
            passengers: Vec::new(),
            vehicle_type: Id::create(&io.vehicle_type),
            attributes: io.attributes.map(Into::into),
        }
    }

    #[cfg(test)]
    pub fn new(
        id: u64,
        veh_type: u64,
        max_v: f32,
        pce: f32,
        driver: Option<SimulationAgent>,
    ) -> Self {
        InternalVehicle {
            id: Id::create(&*id.to_string()),
            max_v: max_v,
            pce,
            driver: None,
            passengers: Vec::new(),
            vehicle_type: Id::create(&*veh_type.to_string()),
            attributes: Default::default(),
        }
    }

    pub fn driver(&self) -> &SimulationAgent {
        self.driver.as_ref().unwrap()
    }

    pub fn passengers(&self) -> &Vec<SimulationAgent> {
        &self.passengers
    }

    pub fn id(&self) -> &Id<InternalVehicle> {
        &self.id
    }

    pub fn register_moved_to_next_link(&mut self) {
        todo!()
    }

    pub fn register_vehicle_exited(&mut self) {
        todo!()
    }

    pub fn route_index_to_last(&mut self) {
        todo!()
    }

    pub fn curr_link_id(&self) -> Option<Id<Link>> {
        todo!()
    }

    pub fn peek_next_route_element(&self) -> Option<Id<Link>> {
        todo!()
    }
}

impl EndTime for InternalVehicle {
    fn end_time(&self, now: u32) -> u32 {
        self.driver().end_time(now)
    }
}

impl From<IOVehicleDefinitions> for InternalGarage {
    fn from(io: IOVehicleDefinitions) -> Self {
        let veh = io
            .vehicles
            .into_iter()
            .map(|v| InternalVehicle::from_io(&io.veh_types, v))
            .collect();
        InternalGarage {
            vehicle_types: io.veh_types.into_iter().map(Into::into).collect(),
            vehicles: veh,
        }
    }
}
