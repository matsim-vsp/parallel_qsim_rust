use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use prost::Message;
use serde::{Deserialize, Deserializer};
use tracing::info;

use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::io::proto::MessageIter;
use crate::simulation::io::{proto, xml};
use crate::simulation::population::population_data::Population;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::population::Header;
use crate::simulation::wire_types::population::Person;

pub fn from_file<F: Fn(&Person) -> bool>(
    path: &Path,
    garage: &mut Garage,
    filter: F,
) -> Population {
    if path.extension().unwrap().eq("binpb") {
        load_from_proto(path, filter)
    } else if path.extension().unwrap().eq("xml") || path.extension().unwrap().eq("gz") {
        let persons = load_from_xml(path, garage)
            .into_iter()
            .filter(|(_id, p)| filter(p))
            .collect();
        Population { persons }
    } else {
        panic!("Tried to load {path:?}. File format not supported. Either use `.xml`, `.xml.gz`, or `.binpb` as extension");
    }
}

pub fn to_file(population: &Population, path: &Path) {
    if path.extension().unwrap().eq("binpb") {
        write_to_proto(population, path);
    } else if path.extension().unwrap().eq("xml") || path.extension().unwrap().eq("gz") {
        write_to_xml(population, path);
    } else {
        panic!("file format not supported. Either use `.xml`, `.xml.gz`, or `.binpb` as extension");
    }
}

fn load_from_xml(path: &Path, garage: &mut Garage) -> HashMap<Id<Person>, Person> {
    let io_pop = IOPopulation::from_file(path.to_str().unwrap());
    create_ids(&io_pop, garage);
    create_population(io_pop)
}

fn write_to_xml(_population: &Population, _path: &Path) {
    panic!("Write to xml is not implemented for Population. Only writing to `.binpb` is supported")
}

fn load_from_proto<F>(path: &Path, filter: F) -> Population
where
    F: Fn(&Person) -> bool,
{
    info!("Loading population from file at: {path:?}");
    let file = File::open(path).unwrap_or_else(|_| panic!("Could not open File at {path:?}"));
    let mut reader = BufReader::new(file);

    if let Some(header_delim) = proto::read_delimiter(&mut reader) {
        let mut buffer = vec![0; header_delim];
        reader
            .read_exact(&mut buffer)
            .expect("Failed to read delimited buffer.");
        let header = Header::decode(buffer.as_slice()).expect("oh nono");
        info!("Header Info: {header:?}");
    }

    let mut persons = HashMap::new();

    for person in MessageIter::<Person, BufReader<File>>::new(reader) {
        let id = Id::get(person.id);
        if filter(&person) {
            persons.insert(id, person);
        }
    }

    Population { persons }
}

fn write_to_proto(population: &Population, path: &Path) {
    info!("Converting Population into wire format");

    let prefix = path.parent().unwrap();
    fs::create_dir_all(prefix).unwrap();
    let file = File::create(path).unwrap_or_else(|_| panic!("Failed to create file at: {path:?}"));
    let mut writer = BufWriter::new(file);
    //write header
    let header = Header {
        version: 1,
        size: population.persons.len() as u32,
    };
    let mut bytes = Vec::new();
    header
        .encode_length_delimited(&mut bytes)
        .expect("TODO: panic message");
    writer.write_all(&bytes).expect("Failed to write");

    for person in population.persons.values() {
        bytes.clear();
        person
            .encode_length_delimited(&mut bytes)
            .expect("Failed to encode person");
        writer.write_all(&bytes).expect("failed to write buffer");
    }

    writer.flush().expect("Failed to flush buffer");
}

fn create_ids(io_pop: &IOPopulation, garage: &mut Garage) {
    info!("Creating person ids.");
    // create person ids and collect strings for vehicle ids
    let raw_veh: Vec<_> = io_pop
        .persons
        .iter()
        .map(|p| Id::<Person>::create(p.id.as_str()))
        .flat_map(|p_id| {
            garage
                .vehicle_types
                .keys()
                .map(move |type_id| (p_id.clone(), type_id.clone()))
        })
        .collect();

    info!("Creating interaction activity types");
    // add interaction activity type for each vehicle type
    for (_, id) in raw_veh.iter() {
        Id::<String>::create(&format!("{} interaction", id.external()));
    }

    info!("Creating vehicle ids");
    for (person_id, type_id) in raw_veh {
        garage.add_veh_by_type(&person_id, &type_id);
    }

    info!("Creating activity types");
    // now iterate over all plans to extract activity ids
    io_pop
        .persons
        .iter()
        .flat_map(|person| person.plans.iter())
        .flat_map(|plan| plan.elements.iter())
        .filter_map(|element| match element {
            IOPlanElement::Activity(a) => Some(a),
            IOPlanElement::Leg(_) => None,
        })
        .map(|act| &act.r#type)
        .for_each(|act_type| {
            Id::<String>::create(act_type.as_str());
        });
}

fn create_population(io_pop: IOPopulation) -> HashMap<Id<Person>, Person> {
    let mut result = HashMap::new();
    for io_person in io_pop.persons {
        let person = Person::from_io(io_person);
        result.insert(Id::get(person.id()), person);
    }
    result
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct IORoute {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@start_link")]
    pub start_link: String,
    #[serde(rename = "@end_link")]
    pub end_link: String,
    #[serde(rename = "@trav_time")]
    pub trav_time: Option<String>,
    #[serde(rename = "@distance")]
    pub distance: f64,
    #[serde(
        rename = "@vehicleRefId",
        default,
        deserialize_with = "option_string_preserve_null"
    )]
    pub vehicle: Option<String>,

    // this needs to be parsed later
    #[serde(rename = "$value")]
    pub route: Option<String>,
}

fn option_string_preserve_null<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(ref s) if s == "null" => Ok(Some("null".to_string())),
        other => Ok(other),
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct IOActivity {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@link")]
    pub link: String,
    #[serde(rename = "@x")]
    pub x: f64,
    #[serde(rename = "@y")]
    pub y: f64,
    #[serde(rename = "@start_time")]
    pub start_time: Option<String>,
    #[serde(rename = "@end_time")]
    pub end_time: Option<String>,
    #[serde(rename = "@max_dur")]
    pub max_dur: Option<String>,
}

impl IOActivity {
    pub fn is_interaction(&self) -> bool {
        self.r#type.contains("interaction")
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct IOLeg {
    #[serde(rename = "@mode")]
    pub mode: String,
    #[serde(rename = "@dep_time")]
    pub dep_time: Option<String>,
    #[serde(rename = "@trav_time")]
    pub trav_time: Option<String>,
    pub route: Option<IORoute>,
    pub attributes: Option<Attrs>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IOPlanElement {
    // the current matsim implementation has more logic with facility-id, link-id and coord.
    // This prototype assumes a fully specified activity with coord and link-id. We don't care about
    // Facilities at this stage.
    Activity(IOActivity),
    Leg(IOLeg),
}

impl IOPlanElement {
    pub fn get_activity(element: Option<&IOPlanElement>) -> Option<&IOActivity> {
        element.and_then(|e| {
            if let IOPlanElement::Activity(activity) = e {
                Some(activity)
            } else {
                None
            }
        })
    }

    pub fn get_leg(element: Option<&IOPlanElement>) -> Option<&IOLeg> {
        element.and_then(|e| {
            if let IOPlanElement::Leg(leg) = e {
                Some(leg)
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct IOPlan {
    #[serde(rename = "@selected", deserialize_with = "bool_from_yes_no")]
    pub selected: bool,
    // https://users.rust-lang.org/t/serde-deserializing-a-vector-of-enums/51647/2
    #[serde(rename = "$value")]
    pub elements: Vec<IOPlanElement>,
}

fn bool_from_yes_no<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.to_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(serde::de::Error::custom(format!("invalid value: {}", s))),
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct IOPerson {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "plan")]
    pub plans: Vec<IOPlan>,
    #[serde(rename = "attributes", skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Attrs>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct IOPopulation {
    #[serde(rename = "person", default)]
    pub persons: Vec<IOPerson>,
}

impl IOPopulation {
    pub fn from_file(file_path: &str) -> IOPopulation {
        let population: IOPopulation = xml::read_from_file(file_path);
        info!(
            "IOPopulation: Finished reading population. Population contains {} persons",
            population.persons.len()
        );
        population
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use quick_xml::de::from_str;

    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::network::global_network::Network;
    use crate::simulation::population::io::{load_from_xml, IOLeg, IOPlanElement, IOPopulation};
    use crate::simulation::population::population_data::Population;
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::wire_types::population::Person;

    /**
    This tests against the first person from the equil scenario. Probably this doesn't cover all
    possibilities and needs to improved later.
     */
    #[test]
    fn read_population_from_string() {
        let xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<!DOCTYPE population SYSTEM \"http://www.matsim.org/files/dtd/population_v6.dtd\">

    <population>
        <attributes>
            <attribute name=\"coordinateReferenceSystem\" class=\"java.lang.String\">Atlantis</attribute>
        </attributes>

        <person id=\"1\">
            <attributes>
                <attribute name=\"vehicles\" class=\"org.matsim.vehicles.PersonVehicles\">{\"car\":\"1\"}</attribute>
            </attributes>
            <plan selected=\"yes\">
                <activity type=\"h\" link=\"1\" x=\"-25000.0\" y=\"0.0\" end_time=\"06:00:00\" >
                </activity>
                <leg mode=\"car\">
                    <attributes>
                        <attribute name=\"routingMode\" class=\"java.lang.String\">car</attribute>
                    </attributes>
                    <route type=\"links\" start_link=\"1\" end_link=\"20\" trav_time=\"undefined\" distance=\"25000.0\" vehicleRefId=\"null\">1 6 15 20</route>
                </leg>
                <activity type=\"w\" link=\"20\" x=\"10000.0\" y=\"0.0\" max_dur=\"00:10:00\" >
                </activity>
                <leg mode=\"car\">
                    <attributes>
                        <attribute name=\"routingMode\" class=\"java.lang.String\">car</attribute>
                    </attributes>
                    <route type=\"links\" start_link=\"20\" end_link=\"20\" trav_time=\"undefined\" distance=\"0.0\" vehicleRefId=\"null\">20</route>
                </leg>
                <activity type=\"w\" link=\"20\" x=\"10000.0\" y=\"0.0\" max_dur=\"03:30:00\" >
                </activity>
                <leg mode=\"car\">
                    <attributes>
                        <attribute name=\"routingMode\" class=\"java.lang.String\">car</attribute>
                    </attributes>
                    <route type=\"links\" start_link=\"20\" end_link=\"1\" trav_time=\"undefined\" distance=\"65000.0\" vehicleRefId=\"null\">20 21 22 23 1</route>
                </leg>
                <activity type=\"h\" link=\"1\" x=\"-25000.0\" y=\"0.0\" >
                </activity>
            </plan>
        </person>

    </population>";

        let population: IOPopulation = from_str(xml).unwrap();

        //test overall structure of population
        assert_eq!(1, population.persons.len());

        let person = population.persons.get(0).unwrap();
        assert_eq!("1", person.id);
        assert_eq!(1, person.plans.len());

        let plan = person.plans.get(0).unwrap();
        assert!(plan.selected);
        assert_eq!(7, plan.elements.len());

        // probe for first leg and second activity
        let leg1 = plan.elements.get(1).unwrap();
        match leg1 {
            IOPlanElement::Activity { .. } => {
                panic!("Plan Element at index 1 was expected to be a leg, but was Activity")
            }
            IOPlanElement::Leg(leg) => {
                // <leg mode=\"car\">
                //     <route type=\"links\" start_link=\"1\" end_link=\"20\" trav_time=\"undefined\" distance=\"25000.0\" vehicleRefId=\"null\">1 6 15 20</route>
                // </leg>
                assert_eq!("car", leg.mode);
                assert_eq!(None, leg.trav_time);
                assert_eq!(None, leg.dep_time);
                let route = leg.route.as_ref().unwrap();
                assert_eq!("links", route.r#type);
                assert_eq!("1", route.start_link);
                assert_eq!("20", route.end_link);
                assert_eq!("undefined", route.trav_time.as_ref().unwrap());
                assert_eq!(25000.0, route.distance);
                assert_eq!("null", route.vehicle.as_ref().unwrap());
                assert_eq!("1 6 15 20", route.route.as_ref().unwrap())
            }
        }

        let activity2 = plan.elements.get(4).unwrap();
        match activity2 {
            IOPlanElement::Activity(activity) => {
                //<activity type=\"w\" link=\"20\" x=\"10000.0\" y=\"0.0\" max_dur=\"03:30:00\" >
                assert_eq!("w", activity.r#type);
                assert_eq!("20", activity.link);
                assert_eq!(10000.0, activity.x);
                assert_eq!(0.0, activity.y);
                assert_eq!(Some(String::from("03:30:00")), activity.max_dur);
                assert_eq!(None, activity.start_time);
                assert_eq!(None, activity.end_time);
            }
            IOPlanElement::Leg { .. } => {
                panic!("Plan element at inded 6 was expected to be an activity but was a Leg.")
            }
        }
    }

    #[test]
    fn test_read_leg() {
        let xml = "<leg mode=\"walk\" dep_time=\"00:00:00\">
                                <attributes>
                                        <attribute name=\"routingMode\" class=\"java.lang.String\">car</attribute>
                                </attributes>
                                <route type=\"generic\" start_link=\"4410448#0\" end_link=\"4410448#0\" trav_time=\"00:00:46\" distance=\"57.23726831365165\"></route>
                        </leg>";

        let leg = from_str::<IOLeg>(xml).unwrap();
        assert_eq!(leg.mode, "walk");
        assert_eq!(leg.dep_time, Some(String::from("00:00:00")));
        assert_eq!(leg.trav_time, None);
        let route = leg.route.as_ref().unwrap();
        assert_eq!(route.r#type, "generic");
        assert_eq!(route.start_link, "4410448#0");
        assert_eq!(route.end_link, "4410448#0");
        assert_eq!(route.trav_time, Some(String::from("00:00:46")));
        assert_eq!(route.distance, 57.23726831365165);
        assert_eq!(route.vehicle, None);
        assert_eq!(route.route, None);
    }

    #[test]
    fn test_read_leg_with_pt() {
        let xml = "<leg mode=\"pt\" trav_time=\"00:10:01\">
				<attributes>
					<attribute name=\"routingMode\" class=\"java.lang.String\">pt</attribute>
				</attributes>
				<route type=\"default_pt\" start_link=\"33\" end_link=\"11\" trav_time=\"00:10:01\" distance=\"NaN\">{\"transitRouteId\":\"3to1\",\"boardingTime\":\"undefined\",\"transitLineId\":\"Blue Line\",\"accessFacilityId\":\"3\",\"egressFacilityId\":\"1\"}</route>
			</leg>";
        let leg = from_str::<IOLeg>(xml).unwrap();
        assert_eq!(leg.mode, "pt");
        assert_eq!(leg.dep_time, None);
        assert_eq!(leg.trav_time, Some(String::from("00:10:01")));
        let route = leg.route.as_ref().unwrap();
        assert_eq!(route.r#type, "default_pt");
        assert_eq!(route.start_link, "33");
        assert_eq!(route.end_link, "11");
        assert_eq!(route.trav_time, Some(String::from("00:10:01")));
        assert!(route.distance.is_nan());
        assert_eq!(route.vehicle, None);
        assert_eq!(route.route, Some(String::from("{\"transitRouteId\":\"3to1\",\"boardingTime\":\"undefined\",\"transitLineId\":\"Blue Line\",\"accessFacilityId\":\"3\",\"egressFacilityId\":\"1\"}")));
    }

    #[test]
    fn read_example_file() {
        let population = IOPopulation::from_file("./assets/population-v6-34-persons.xml");
        assert_eq!(34, population.persons.len())
    }

    #[test]
    fn read_example_file_gzipped() {
        let population = IOPopulation::from_file("./assets/population-v6-34-persons.xml.gz");
        assert_eq!(34, population.persons.len())
    }

    #[test]
    fn test_conversion() {
        let _net = Network::from_file(
            "./assets/equil/equil-network.xml",
            2,
            PartitionMethod::Metis(MetisOptions::default()),
        );
        let mut garage = Garage::from_file(&PathBuf::from("./assets/equil/equil-vehicles.xml"));

        let persons = load_from_xml(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &mut garage,
        );
        assert_eq!(persons.len(), 100);

        for i in 1u32..101 {
            assert!(persons.get(&Id::get_from_ext(&format!("{}", i))).is_some());
        }
    }

    #[test]
    fn test_proto() {
        let _net = Network::from_file_as_is(&PathBuf::from("./assets/equil/equil-network.xml"));
        let mut garage = Garage::from_file(&PathBuf::from("./assets/equil/equil-vehicles.xml"));
        let pop = Population::from_file(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &mut garage,
        );

        let file_path =
            PathBuf::from("./test_output/simulation/population/io/test_proto/plans.binpb");
        pop.to_file(&file_path);

        let proto_pop = Population::from_file(&file_path, &mut garage);

        for (id, person) in pop.persons {
            assert!(proto_pop.persons.contains_key(&id));
            let proto_person = proto_pop.persons.get(&id).unwrap();
            assert_eq!(person.id, proto_person.id);
        }
    }

    #[test]
    fn test_filtered_proto() {
        let _net = Network::from_file_as_is(&PathBuf::from("./assets/equil/equil-network.xml"));
        let mut garage = Garage::from_file(&PathBuf::from("./assets/equil/equil-vehicles.xml"));
        let pop = Population::from_file(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &mut garage,
        );

        let file_path =
            PathBuf::from("./test_output/simulation/population/io/test_proto/plans.binpb");
        pop.to_file(&file_path);

        let proto_pop = Population::from_file_filtered(&file_path, &mut garage, |p| p.id == 1);

        let expected_id: Id<Person> = Id::get(1);
        assert_eq!(1, proto_pop.persons.len());
        assert!(proto_pop.persons.contains_key(&expected_id));
    }
}
