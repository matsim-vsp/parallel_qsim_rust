use serde::{Deserialize, Serialize};

use crate::simulation::io::attributes::Attrs;
use crate::simulation::io::xml_reader;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "vehicleDefinitions")]
pub struct IOVehicleDefinitions {
    #[serde(rename = "vehicleType")]
    veh_types: Vec<IOVehicleType>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "vehicleDefinitions")]
pub struct IOVehicles {
    #[serde(rename = "vehicleType", default)]
    vehicle_types: Vec<IOVehicleType>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IOVehicleType {
    pub id: String,
    pub descr: String,
    pub capacity: IOCapacity,
    pub length: f32,
    pub width: f32,
    pub maximum_velocity: f32,
    pub engine_information: IOEngineInformation,
    pub cost_information: IOCostInformation,
    pub passenger_car_equivalents: f32,
    pub network_mode: String,
    pub flow_efficiency_factor: f32,
    pub attributes: Option<Attrs>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "capacity")]
pub struct IOCapacity {
    // leave emtpy for now
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "capacity")]
pub struct IOEngineInformation {
    // leave empty for now.
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "capacity")]
pub struct IOCostInformation {
    // leave empty for now.
}

impl IOVehicleDefinitions {
    pub fn from_file(file: &str) -> Self {
        xml_reader::read(file)
    }
}

#[cfg(test)]
mod test {
    use crate::simulation::io::vehicles::IOVehicleDefinitions;

    #[test]
    fn test_from_file() {
        let io_vehicles = IOVehicleDefinitions::from_file("./assets/vehicles/vehicles_v2.xml");
        println!("{io_vehicles:#?}")
    }
}
