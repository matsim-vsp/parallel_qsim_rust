use serde::{Deserialize, Serialize};

use crate::simulation::io::attributes::Attrs;
use crate::simulation::io::xml_reader;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "vehicleDefinitions")]
pub struct IOVehicleDefinitions {
    #[serde(rename = "vehicleType")]
    pub veh_types: Vec<IOVehicleType>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "vehicleDefinitions")]
pub struct IOVehicles {
    #[serde(rename = "vehicleType", default)]
    vehicle_types: Vec<IOVehicleType>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOVehicleType {
    pub id: String,
    pub description: Option<String>,
    pub capacity: Option<IOCapacity>,
    pub length: Option<IODimension>,
    pub width: Option<IODimension>,
    pub maximum_velocity: Option<IOVelocity>,
    pub engine_information: Option<IOEngineInformation>,
    pub cost_information: Option<IOCostInformation>,
    pub passenger_car_equivalents: Option<IOPassengerCarEquivalents>,
    pub network_mode: Option<IONetworkMode>,
    pub flow_efficiency_factor: Option<IOFowEfficiencyFactor>,
    pub attributes: Option<Attrs>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IOCapacity {
    // leave emtpy for now
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IODimension {
    pub(crate) meter: f32,
}

impl Default for IODimension {
    fn default() -> Self {
        Self { meter: 1. }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOVelocity {
    pub(crate) meter_per_second: f32,
}

impl Default for IOVelocity {
    fn default() -> Self {
        Self {
            meter_per_second: f32::MAX,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IOPassengerCarEquivalents {
    pub(crate) pce: f32,
}

impl Default for IOPassengerCarEquivalents {
    fn default() -> Self {
        Self { pce: 1. }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IONetworkMode {
    pub(crate) network_mode: String,
}

impl Default for IONetworkMode {
    fn default() -> Self {
        Self {
            network_mode: String::from("car"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IOFowEfficiencyFactor {
    pub(crate) factor: f32,
}

impl Default for IOFowEfficiencyFactor {
    fn default() -> Self {
        Self { factor: 1. }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IOEngineInformation {
    // leave empty for now.
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
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
    use quick_xml::de::from_str;

    use crate::simulation::io::vehicles::IOVehicleDefinitions;

    #[test]
    fn from_string_empty_type() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
                            <vehicleDefinitions xmlns=\"http://www.matsim.org/files/dtd\">\
                                <vehicleType id=\"some-vehicle-id\">\
                                </vehicleType>\
                            </vehicleDefinitions>\
                        ";
        let veh_def: IOVehicleDefinitions = from_str(xml).unwrap();

        assert_eq!(1, veh_def.veh_types.len());

        let veh_type = veh_def.veh_types.first().unwrap();
        assert_eq!("some-vehicle-id", veh_type.id.as_str());
    }

    #[test]
    fn from_string_full_type() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
                            <vehicleDefinitions xmlns=\"http://www.matsim.org/files/dtd\">\
                                <vehicleType id=\"some-vehicle-id\">\
                                    <description>some-description</description>\
                                    <length meter=\"9.5\"/>\
                                    <width meter=\"9.5\"/>\
                                    <maximumVelocity meterPerSecond=\"9.5\"/>\
                                    <passengerCarEquivalents pce=\"9.5\"/>\
                                    <networkMode networkMode=\"some-network-mode\"/>\
                                    <flowEfficiencyFactor factor=\"9.5\"/>\
                                </vehicleType>\
                            </vehicleDefinitions>\
                        ";

        let veh_def: IOVehicleDefinitions = from_str(xml).unwrap();

        assert_eq!(1, veh_def.veh_types.len());

        let veh_type = veh_def.veh_types.first().unwrap();
        assert_eq!("some-vehicle-id", veh_type.id.as_str());
        assert_eq!(
            "some-description",
            veh_type.description.as_ref().unwrap().as_str()
        );
        assert_eq!(
            "some-network-mode",
            veh_type
                .network_mode
                .as_ref()
                .unwrap()
                .network_mode
                .as_str()
        );
        assert_eq!(9.5, veh_type.length.as_ref().unwrap().meter);
        assert_eq!(9.5, veh_type.width.as_ref().unwrap().meter);
        assert_eq!(
            9.5,
            veh_type.maximum_velocity.as_ref().unwrap().meter_per_second
        );
        assert_eq!(
            9.5,
            veh_type.passenger_car_equivalents.as_ref().unwrap().pce
        );
        assert_eq!(
            9.5,
            veh_type.flow_efficiency_factor.as_ref().unwrap().factor
        );
    }

    #[test]
    fn test_from_file() {
        let veh_def = IOVehicleDefinitions::from_file("./assets/vehicles/vehicles_v2.xml");
        assert_eq!(3, veh_def.veh_types.len());
        // no further assertions here, as the tests above test the individual properties.
        // so, this test mainly tests whether the vehicles implementation calls the xml_reader
    }
}
