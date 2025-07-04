use crate::generated::vehicles::{Vehicle, VehicleType};
use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::agents::{AgentEvent, EnvironmentalEventObserver, SimulationAgentLogic};
use crate::simulation::id::Id;
use crate::simulation::io::proto::proto_vehicles::{load_from_proto, write_to_proto};
use crate::simulation::io::xml::vehicles::{load_from_xml, write_to_xml, IOVehicle, IOVehicleType};
use crate::simulation::network::Link;
use crate::simulation::time_queue::EndTime;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::InternalAttributes;
use std::fmt::Debug;
use std::path::Path;

pub mod garage;

pub fn from_file(path: &Path) -> Garage {
    if path.extension().unwrap().eq("binpb") {
        load_from_proto(path)
    } else if path.extension().unwrap().eq("xml") || path.extension().unwrap().eq("gz") {
        load_from_xml(path)
    } else {
        panic!("Tried to load {path:?}. File format not supported. Either use `.xml`, `.xml.gz`, or `.binpb` as extension");
    }
}

pub fn to_file(garage: &Garage, path: &Path) {
    if path.extension().unwrap().eq("binpb") {
        write_to_proto(garage, path);
    } else if path.extension().unwrap().eq("xml") || path.extension().unwrap().eq("gz") {
        write_to_xml(garage, path);
    } else {
        panic!("file format not supported. Either use `.xml`, `.xml.gz`, or `.binpb` as extension");
    }
}

#[derive(Debug, PartialEq)]
pub struct InternalVehicleType {
    pub id: Id<InternalVehicleType>,
    pub length: f32,
    pub width: f32,
    pub max_v: f32,
    pub pce: f32,
    pub fef: f32,
    pub net_mode: Id<String>,
    pub attributes: InternalAttributes,
}

#[derive(Debug, PartialEq)]
pub struct InternalVehicle {
    pub id: Id<InternalVehicle>,
    pub max_v: f32,
    pub pce: f32,
    pub driver: Option<SimulationAgent>,
    pub passengers: Vec<SimulationAgent>,
    pub vehicle_type: Id<InternalVehicleType>,
    pub attributes: InternalAttributes,
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
            attributes: io.attributes.map(Into::into).unwrap_or_default(),
        }
    }
}

impl From<VehicleType> for InternalVehicleType {
    fn from(value: VehicleType) -> Self {
        Self {
            id: Id::get(value.id),
            length: value.length,
            width: value.width,
            max_v: value.max_v,
            pce: value.pce,
            fef: value.fef,
            net_mode: Id::get(value.net_mode),
            attributes: InternalAttributes::default(),
        }
    }
}

impl From<Vehicle> for InternalVehicle {
    fn from(value: Vehicle) -> Self {
        Self {
            id: Id::get(value.id),
            max_v: value.max_v,
            pce: value.pce,
            driver: None,
            passengers: vec![],
            vehicle_type: Id::get(value.r#type),
            attributes: InternalAttributes::from(value.attributes),
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
            attributes: io.attributes.map(Into::into).unwrap_or_default(),
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
            id: Id::create(&id.to_string()),
            max_v,
            pce,
            driver,
            passengers: Vec::new(),
            vehicle_type: Id::create(&veh_type.to_string()),
            attributes: Default::default(),
        }
    }

    fn driver_mut(&mut self) -> &mut SimulationAgent {
        self.driver.as_mut().unwrap()
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

    pub fn curr_link_id(&self) -> Option<&Id<Link>> {
        self.driver().curr_link_id()
    }

    pub fn peek_next_route_element(&self) -> Option<&Id<Link>> {
        self.driver().peek_next_link_id()
    }
}

impl EnvironmentalEventObserver for InternalVehicle {
    fn notify_event(&mut self, event: AgentEvent, now: u32) {
        self.driver_mut().notify_event(event.clone(), now);
        self.passengers.iter_mut().for_each(|p| {
            p.notify_event(event.clone(), now);
        });
        // todo!("Better provide event as &mut?")
    }
}

impl EndTime for InternalVehicle {
    fn end_time(&self, now: u32) -> u32 {
        self.driver().end_time(now)
    }
}
