use crate::simulation::id::Id;
use crate::simulation::network::global_network::Link;
use crate::simulation::time_queue::EndTime;
use crate::simulation::vehicles::io::{IOVehicle, IOVehicleType};
use crate::simulation::{InternalAttributes, InternalSimulationAgent};
use itertools::Itertools;
use std::fmt::Debug;

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
    pub driver: Option<InternalSimulationAgent>,
    pub passengers: Vec<InternalSimulationAgent>,
    pub vehicle_type: Id<InternalVehicleType>,
    pub attributes: Option<InternalAttributes>,
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
    pub fn from_io(io: IOVehicle, io_veh_type: &InternalVehicleType) -> Self {
        InternalVehicle {
            id: Id::create(&io.id),
            max_v: io_veh_type.max_v,
            pce: io_veh_type.pce,
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
        driver: Option<InternalSimulationAgent>,
    ) -> Self {
        InternalVehicle {
            id: Id::create(&*id.to_string()),
            max_v: max_v,
            pce,
            driver,
            passengers: Vec::new(),
            vehicle_type: Id::create(&*veh_type.to_string()),
            attributes: Default::default(),
        }
    }

    fn driver_mut(&mut self) -> &mut InternalSimulationAgent {
        self.driver.as_mut().unwrap()
    }

    pub fn driver(&self) -> &InternalSimulationAgent {
        self.driver.as_ref().unwrap()
    }

    pub fn passengers(&self) -> &Vec<InternalSimulationAgent> {
        &self.passengers
    }

    pub fn id(&self) -> &Id<InternalVehicle> {
        &self.id
    }

    pub fn register_moved_to_next_link(&mut self) {
        self.driver_mut().register_moved_to_next_link();
    }

    pub fn register_vehicle_exited(&mut self) {
        self.driver_mut().register_vehicle_exited();
    }

    pub fn route_index_to_last(&mut self) {
        self.driver_mut().route_index_to_last();
    }

    pub fn curr_link_id(&self) -> Option<&Id<Link>> {
        self.driver().curr_link_id()
    }

    pub fn peek_next_route_element(&self) -> Option<&Id<Link>> {
        self.driver().peek_next_link_id()
    }
}

impl EndTime for InternalVehicle {
    fn end_time(&self, now: u32) -> u32 {
        self.driver().end_time(now)
    }
}
