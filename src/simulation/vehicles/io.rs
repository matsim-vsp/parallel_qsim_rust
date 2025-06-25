use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::io::xml;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::vehicles::InternalVehicleType;
use crate::simulation::wire_types::messages::Vehicle;
use crate::simulation::wire_types::vehicles::{VehicleType, VehiclesContainer};

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

fn load_from_xml(path: &Path) -> Garage {
    let io_vehicles = IOVehicleDefinitions::from_file(path.to_str().unwrap());
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

fn write_to_xml(garage: &Garage, path: &Path) {
    info!("Converting Garage into xml type");

    let veh_types = garage
        .vehicle_types
        .values()
        .map(|t| IOVehicleType {
            id: t.id.external().to_owned(),
            description: None,
            capacity: None,
            length: Some(IODimension { meter: t.length }),
            width: Some(IODimension { meter: t.width }),
            maximum_velocity: Some(IOVelocity {
                meter_per_second: t.max_v,
            }),
            engine_information: None,
            cost_information: None,
            passenger_car_equivalents: Some(IOPassengerCarEquivalents { pce: t.pce }),
            network_mode: Some(IONetworkMode {
                network_mode: t.net_mode.external().to_owned(),
            }),
            flow_efficiency_factor: Some(IOFowEfficiencyFactor { factor: t.fef }),
            attributes: None,
        })
        .collect();

    let io_vehicles = IOVehicleDefinitions {
        veh_types,
        vehicles: vec![],
    };

    xml::write_to_file(&io_vehicles, path, "<!DOCTYPE network SYSTEM \"http://www.matsim.org/files/dtd http://www.matsim.org/files/dtd/vehicleDefinitions_v2.0.xsd\">")
}

fn write_to_proto(garage: &Garage, path: &Path) {
    // info!("Converting Garage into wire type");
    // let vehicle_types = garage.vehicle_types.values().cloned().collect();
    // let vehicles = garage.vehicles.values().cloned().collect();
    //
    // let wire_format = VehiclesContainer {
    //     vehicle_types,
    //     vehicles,
    // };
    // info!("Finished converting Garage into wire type");
    // simulation::io::proto::write_to_file(wire_format, path);
    unimplemented!()
}

fn load_from_proto(path: &Path) -> Garage {
    // let wire_garage: VehiclesContainer = simulation::io::proto::read_from_file(path);
    // let vehicles = wire_garage
    //     .vehicles
    //     .into_iter()
    //     .map(|v| (Id::<Vehicle>::get(v.id), v))
    //     .collect();
    // let vehicle_types = wire_garage
    //     .vehicle_types
    //     .into_iter()
    //     .map(|v_type| (Id::get(v_type.id), v_type))
    //     .collect();
    // Garage {
    //     vehicles,
    //     vehicle_types,
    // }
    unimplemented!()
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
        attributes: io_veh_type.attributes.map(Into::into),
    };
    garage.add_veh_type(veh_type);
}

fn add_io_veh(garage: &mut Garage, io_veh: IOVehicle) {
    let veh_type = garage.vehicle_types.get(&Id::get_from_ext(io_veh.vehicle_type.as_str()))
        .expect("Vehicle type of vehicle not found. There has to be a vehicle type defined before a vehicle can be added.");
    let vehicle = Vehicle::from_io(io_veh, veh_type);

    //add id for drt mode
    if let Some(o) = vehicle
        .attributes
        .as_ref()
        .unwrap()
        .get::<String>("dvrpMode")
    {
        Id::<String>::create(o.as_str());
    }

    garage.add_veh(vehicle);
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "vehicleDefinitions")]
pub struct IOVehicleDefinitions {
    #[serde(rename = "vehicleType")]
    pub veh_types: Vec<IOVehicleType>,
    #[serde(rename = "vehicle", default)]
    pub vehicles: Vec<IOVehicle>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IOVehicle {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub vehicle_type: String,
    pub attributes: Option<Attrs>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IOVehicleType {
    #[serde(rename = "@id")]
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
    #[serde(rename = "@meter")]
    pub(crate) meter: f32,
}

impl Default for IODimension {
    fn default() -> Self {
        Self { meter: 1. }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct IOVelocity {
    #[serde(rename = "@meterPerSecond")]
    pub(crate) meter_per_second: f32,
}

impl Default for IOVelocity {
    fn default() -> Self {
        Self {
            meter_per_second: f32::MAX,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Copy)]
pub struct IOPassengerCarEquivalents {
    #[serde(rename = "@pce")]
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
    #[serde(rename = "@networkMode")]
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
    #[serde(rename = "@factor")]
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
        xml::read_from_file(file)
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use quick_xml::de::from_str;

    use crate::simulation::id::Id;
    use crate::simulation::io::attributes::{Attr, Attrs};
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::vehicles::io::{
        add_io_veh_type, from_file, to_file, IODimension, IOFowEfficiencyFactor, IONetworkMode,
        IOPassengerCarEquivalents, IOVehicleDefinitions, IOVehicleType, IOVelocity,
    };
    use crate::simulation::vehicles::InternalVehicleType;
    use crate::simulation::wire_types::vehicles::VehicleType;

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
                                <vehicle id=\"drt\" type=\"some-vehicle-id\">
                                    <attributes>
                                        <attribute name=\"dvrpMode\" class=\"java.lang.String\">drt</attribute>
                                        <attribute name=\"startLink\" class=\"java.lang.String\">42</attribute>
                                        <attribute name=\"serviceBeginTime\" class=\"java.lang.Double\">0</attribute>
                                        <attribute name=\"serviceEndTime\" class=\"java.lang.Double\">84000</attribute>
                                    </attributes>
                                </vehicle>
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

        let vehicle = veh_def.vehicles.first().unwrap();
        assert_eq!("drt", vehicle.id.as_str());
        assert_eq!("some-vehicle-id", vehicle.vehicle_type.as_str());
        let attrs = vehicle.attributes.as_ref().unwrap();
        assert_eq!(4, attrs.attributes.len());
        assert_eq!("drt", attrs.find_or_else("dvrpMode", || ""));
        assert_eq!("42", attrs.find_or_else("startLink", || ""));
        assert_eq!("0", attrs.find_or_else("serviceBeginTime", || ""));
        assert_eq!("84000", attrs.find_or_else("serviceEndTime", || ""));
    }

    #[test]
    fn test_to_from_file_xml() {
        let file = &PathBuf::from(
            "./test_output/simulation/vehicles/io/test_to_from_file_xml/vehicles.xml",
        );
        let mut garage = Garage::new();

        garage.add_veh_type(InternalVehicleType {
            id: Id::create("some-type"),
            length: 10.,
            width: 20.0,
            max_v: 1000.0,
            pce: 20.0,
            fef: 0.3,
            net_mode: Id::<String>::create("some network type 🚕"),
            attributes: None,
        });
        garage.add_veh_by_type(&Id::create("some-person"), &Id::get_from_ext("some-type"));

        to_file(&garage, file);
        let loaded_garage = from_file(file);
        assert_eq!(garage.vehicle_types, loaded_garage.vehicle_types);
    }

    #[test]
    fn test_to_from_file_proto() {
        let file = &PathBuf::from(
            "./test_output/simulation/vehicles/io/test_to_from_file_xml/vehicles.binpb",
        );
        let mut garage = Garage::new();

        garage.add_veh_type(InternalVehicleType {
            id: Id::create("some-type"),
            length: 10.,
            width: 20.0,
            max_v: 1000.0,
            pce: 20.0,
            fef: 0.3,
            net_mode: Id::<String>::create("some network type 🚕"),
            attributes: None,
        });
        garage.add_veh_by_type(&Id::create("some-person"), &Id::get_from_ext("some-type"));

        to_file(&garage, file);
        let loaded_garage = from_file(file);

        assert_eq!(garage.vehicle_types, loaded_garage.vehicle_types);
        assert_eq!(garage.vehicles, loaded_garage.vehicles);
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
        assert_eq!(veh_type.max_v, 100.);
        assert_eq!(veh_type.width, 5.0);
        assert_eq!(veh_type.length, 10.);
        assert_eq!(veh_type.pce, 21.);
        assert_eq!(veh_type.fef, 2.);
        assert_eq!(veh_type.id.internal(), expected_id.internal());
        assert_eq!(veh_type.net_mode.internal(), expected_mode.internal())
    }
}
