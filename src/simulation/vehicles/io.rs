use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::io::xml_reader;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::vehicles::{LevelOfDetail, VehicleType};

pub fn from_file(path: &Path) -> Garage {
    if path.extension().unwrap().eq("binpb") {
        load_from_proto(path)
    } else if path.extension().unwrap().eq("xml") || path.extension().unwrap().eq("gz") {
        load_from_xml(path)
    } else {
        panic!("Tried to load {path:?}. File format not supported. Either use `.xml`, `.xml.gz`, or `.binpb` as extension");
    }
}

fn load_from_xml(path: &Path) -> Garage {
    let io_vehicles = IOVehicleDefinitions::from_file(path.to_str().unwrap());
    let mut result = Garage::new();
    for io_veh_type in io_vehicles.veh_types {
        add_io_veh_type(&mut result, io_veh_type);
    }
    let keys_ext: Vec<_> = result.vehicle_types.keys().map(|k| k.external()).collect();
    info!(
        "Created Garage from file with vehicle types: {:?}",
        keys_ext
    );
    result
}

fn load_from_proto(path: &Path) -> Garage {
    panic!("Not yet implemented")
}

fn add_io_veh_type(garage: &mut Garage, io_veh_type: IOVehicleType) {
    let id: Id<VehicleType> = Id::create(&io_veh_type.id);
    let net_mode: Id<String> =
        Id::create(&io_veh_type.network_mode.unwrap_or_default().network_mode);
    let lod = if let Some(attr) = io_veh_type
        .attributes
        .unwrap_or_default()
        .attributes
        .iter()
        .find(|&attr| attr.name.eq("lod"))
    {
        match attr.value.to_lowercase().as_str() {
            "teleported" => LevelOfDetail::Teleported,
            "network" => LevelOfDetail::Network,
            _ => LevelOfDetail::Network,
        }
    } else {
        LevelOfDetail::Network
    };

    let veh_type = VehicleType {
        id: id.internal(),
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
        net_mode: net_mode.internal(),
        lod: lod as i32,
    };
    garage.add_veh_type(veh_type);
}

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

    use crate::simulation::id::Id;
    use crate::simulation::io::attributes::{Attr, Attrs};
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::vehicles::io::{
        add_io_veh_type, IODimension, IOFowEfficiencyFactor, IONetworkMode,
        IOPassengerCarEquivalents, IOVehicleDefinitions, IOVehicleType, IOVelocity,
    };
    use crate::simulation::wire_types::vehicles::{LevelOfDetail, VehicleType};

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
        let veh_def = IOVehicleDefinitions::from_file("./assets/3-links/vehicles.xml");
        assert_eq!(3, veh_def.veh_types.len());
        // no further assertions here, as the tests above test the individual properties.
        // so, this test mainly tests whether the vehicles implementation calls the xml_reader
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
        assert_eq!(0, Id::<String>::get_from_ext("car").internal());
        assert_eq!(0, Id::<VehicleType>::get_from_ext("some-id").internal());

        let veh_type_opt = garage.vehicle_types.values().next();
        assert!(veh_type_opt.is_some());
        let veh_type = veh_type_opt.unwrap();
        assert!(matches!(veh_type.lod(), LevelOfDetail::Network));
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
            attributes: Some(Attrs {
                attributes: vec![Attr::new(String::from("lod"), String::from("teleported"))],
            }),
        };
        let mut garage = Garage::new();
        add_io_veh_type(&mut garage, io_veh_type);

        let expected_id: Id<VehicleType> = Id::get_from_ext("some-id");
        let expected_mode: Id<String> = Id::get_from_ext("some_mode");

        let veh_type_opt = garage.vehicle_types.values().next();
        assert!(veh_type_opt.is_some());
        let veh_type = veh_type_opt.unwrap();
        assert!(matches!(veh_type.lod(), LevelOfDetail::Teleported));
        assert_eq!(veh_type.max_v, 100.);
        assert_eq!(veh_type.width, 5.0);
        assert_eq!(veh_type.length, 10.);
        assert_eq!(veh_type.pce, 21.);
        assert_eq!(veh_type.fef, 2.);
        assert_eq!(veh_type.id, expected_id.internal());
        assert_eq!(veh_type.net_mode, expected_mode.internal())
    }
}
