use crate::simulation::io::xml_reader;
use log::debug;
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
    NetworkMode(IONetworkMode),
    FlowEfficiencyFactor(IOFlowEfficiencyFactor),
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

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IONetworkMode {
    network_mode: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOFlowEfficiencyFactor {
    network_mode: f32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VehicleType {
    pub id: String,
    pub maximum_velocity: Option<f32>,
    pub network_mode: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VehicleDefinitions {
    pub vehicle_types: Vec<VehicleType>,
}

impl IOVehicleDefinitions {
    pub fn from_file(file_path: &str) -> IOVehicleDefinitions {
        xml_reader::read::<IOVehicleDefinitions>(file_path)
    }
}

impl VehicleDefinitions {
    pub fn new() -> VehicleDefinitions {
        VehicleDefinitions {
            vehicle_types: vec![],
        }
    }

    pub fn add_vehicle_type(
        mut self,
        id: String,
        maximum_velocity: Option<f32>,
        network_mode: String,
    ) -> VehicleDefinitions {
        self.vehicle_types.push(VehicleType {
            id,
            maximum_velocity,
            network_mode,
        });
        self
    }

    pub fn from_io(io: IOVehicleDefinitions) -> VehicleDefinitions {
        let vehicle_types = io
            .vehicle_types
            .into_iter()
            .map(|t| VehicleDefinitions::convert_io_vehicle_type(t))
            .collect();
        VehicleDefinitions { vehicle_types }
    }

    fn convert_io_vehicle_type(io: IOVehicleType) -> VehicleType {
        VehicleType {
            id: io.id.clone(),
            maximum_velocity: Self::extract_maximum_velocity(&io),
            network_mode: Self::extract_network_mode(&io).unwrap_or_else(|| {
                debug!("There was no specific network mode for vehicle type {}. Using id as network mode.", io.id);
                io.id
            }),
        }
    }

    fn extract_maximum_velocity(io: &IOVehicleType) -> Option<f32> {
        io.attributes
            .iter()
            .filter_map(|a| match a {
                IOVehicleAttribute::MaximumVelocity(v) => Some(v.meter_per_second),
                _ => None,
            })
            .collect::<Vec<f32>>()
            .get(0)
            .cloned()
    }

    fn extract_network_mode(io: &IOVehicleType) -> Option<String> {
        io.attributes
            .iter()
            .filter_map(|a| match a {
                IOVehicleAttribute::NetworkMode(m) => Some(m.network_mode.clone()),
                _ => None,
            })
            .collect::<Vec<String>>()
            .get(0)
            .cloned()
    }

    pub fn get_max_speed_for_mode(&self, mode: &str) -> Option<f32> {
        let mode_vehicle_type = self
            .vehicle_types
            .iter()
            .filter(|&v| v.network_mode.eq(mode))
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
        IOVehicleDefinitions, VehicleDefinitions, VehicleType,
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
                    <networkMode networkMode="car"/>
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

        let vehicle_definitions = VehicleDefinitions::from_io(io_definitions);
        assert_eq!(
            vehicle_definitions,
            VehicleDefinitions {
                vehicle_types: vec![
                    VehicleType {
                        id: "car".to_string(),
                        maximum_velocity: Some(16.67),
                        network_mode: "car".to_string()
                    },
                    VehicleType {
                        id: "bicycle".to_string(),
                        maximum_velocity: Some(4.17),
                        network_mode: "bicycle".to_string()
                    }
                ],
            }
        );
    }
}
