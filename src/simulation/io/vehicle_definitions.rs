use crate::simulation::io::xml_reader;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "vehicleDefinitions")]
pub struct IOVehicleDefinitions {
    #[serde(rename = "vehicleType", default)]
    vehicle_types: Vec<IOVehicleType>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IOVehicleType {
    pub id: String,
    #[serde(rename = "$value")]
    pub attributes: Vec<IOVehicleAttribute>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum IOVehicleAttribute {
    Length(IOLength),
    Width(IOWidth),
    MaximumVelocity(IOMaximumVelocity),
    AccessTime(IOAccessTime),
    EgressTime(IOEgressTime),
    DoorOperation(IODoorOperation),
    PassengerCarEquivalents(IOPassengerCarEquivalents),
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOLength {
    meter: f32,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOWidth {
    meter: f32,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOMaximumVelocity {
    meter_per_second: f32,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOAccessTime {
    seconds_per_person: f32,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOEgressTime {
    seconds_per_person: f32,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IODoorOperation {
    mode: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOPassengerCarEquivalents {
    pce: f32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VehicleType {
    pub id: String,
    pub maximum_velocity: Option<f32>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VehicleTypeDefinitions {
    pub vehicle_types: Vec<VehicleType>,
}

impl IOVehicleDefinitions {
    pub fn from_file(file_path: &str) -> IOVehicleDefinitions {
        xml_reader::read::<IOVehicleDefinitions>(file_path)
    }
}

impl VehicleTypeDefinitions {
    pub fn from_io(io: IOVehicleDefinitions) -> VehicleTypeDefinitions {
        let vehicle_types = io
            .vehicle_types
            .into_iter()
            .map(|t| VehicleTypeDefinitions::convert_io_vehicle_type(t))
            .collect();
        VehicleTypeDefinitions { vehicle_types }
    }

    fn convert_io_vehicle_type(io: IOVehicleType) -> VehicleType {
        let maximum_velocity = io
            .attributes
            .iter()
            .filter_map(|a| match a {
                IOVehicleAttribute::MaximumVelocity(v) => Some(v.meter_per_second),
                _ => None,
            })
            .collect::<Vec<f32>>()
            .get(0)
            .cloned();

        VehicleType {
            id: io.id,
            maximum_velocity,
        }
    }

    pub fn get_max_speed_for_mode(&self, mode: &str) -> Option<f32> {
        let mode_vehicle_type = self
            .vehicle_types
            .iter()
            .filter(|&v| v.id.eq(mode))
            .collect::<Vec<&VehicleType>>();

        if mode_vehicle_type.len() == 0 {
            panic!("There is no vehicle type definition for mode {:?} ", mode)
        } else if mode_vehicle_type.len() > 1 {
            panic!(
                "There are multiple vehicle type definitions for mode {:?} ",
                mode
            )
        }

        mode_vehicle_type.get(0).unwrap().maximum_velocity
    }
}

#[cfg(test)]
mod test {
    use crate::simulation::io::vehicle_definitions::{
        IOVehicleDefinitions, VehicleType, VehicleTypeDefinitions,
    };
    use quick_xml::de::from_str;

    #[test]
    fn test() {
        let io_definitions = from_str::<IOVehicleDefinitions>(
            r#"
            <vehicleDefinitions xmlns="http://www.matsim.org/files/dtd" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://www.matsim.org/files/dtd http://www.matsim.org/files/dtd/vehicleDefinitions_v1.0.xsd">
                <vehicleType id="car">
                    <length meter="7.5"/>
                    <width meter="1.0"/>
                    <maximumVelocity meterPerSecond="16.67"/>
                    <accessTime secondsPerPerson="1.0"/>
                    <egressTime secondsPerPerson="1.0"/>
                    <doorOperation mode="serial"/>
                    <passengerCarEquivalents pce="1.0"/>
                </vehicleType>
                <vehicleType id="bicycle">
                    <length meter="7.5"/>
                    <width meter="1.0"/>
                    <maximumVelocity meterPerSecond="4.17"/>
                    <accessTime secondsPerPerson="1.0"/>
                    <egressTime secondsPerPerson="1.0"/>
                    <doorOperation mode="serial"/>
                    <passengerCarEquivalents pce="0.25"/>
                </vehicleType>
            </vehicleDefinitions>
            "#
        ).unwrap();

        let vehicle_type_definitions = VehicleTypeDefinitions::from_io(io_definitions);
        assert_eq!(
            vehicle_type_definitions,
            VehicleTypeDefinitions {
                vehicle_types: vec![
                    VehicleType {
                        id: "car".to_string(),
                        maximum_velocity: Some(16.67),
                    },
                    VehicleType {
                        id: "bicycle".to_string(),
                        maximum_velocity: Some(4.17)
                    }
                ],
            }
        );
    }
}
