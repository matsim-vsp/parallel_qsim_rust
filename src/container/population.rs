use crate::container::xml_reader;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Route {
    r#type: String,
    start_link: String,
    end_link: String,
    trav_time: Option<String>,
    distance: f32,
    #[serde(rename = "vehicleRefId")]
    vehicle: Option<String>,

    // this needs to be parsed later
    #[serde(rename = "$value")]
    route: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum PlanElement {
    // the current matsim implementation has more logic with facility-id, link-id and coord.
    // This prototype assumes a fully specified activity with coord and link-id. We don't care about
    // Facilities at this stage.
    Activity {
        r#type: String,
        link: String,
        x: f32,
        y: f32,
        start_time: Option<String>,
        end_time: Option<String>,
        max_dur: Option<String>,
    },
    Leg {
        mode: String,
        dep_time: Option<String>,
        trav_time: Option<String>,
        route: Route,
    },
}

#[derive(Debug, Deserialize, PartialEq)]
struct Plan {
    selected: bool,
    // https://users.rust-lang.org/t/serde-deserializing-a-vector-of-enums/51647/2
    #[serde(rename = "$value")]
    elements: Vec<PlanElement>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Person {
    id: String,
    #[serde(rename = "plan")]
    plans: Vec<Plan>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Population {
    #[serde(rename = "person")]
    persons: Vec<Person>,
}

impl Population {
    fn from_file(file_path: &str) -> Population {
        xml_reader::read(file_path)
    }
}

#[cfg(test)]
mod tests {
    use crate::container::population::{PlanElement, Population};
    use quick_xml::de::from_str;

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

        let population: Population = from_str(xml).unwrap();

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
            PlanElement::Activity { .. } => {
                panic!("Plan Element at index 1 was expected to be a leg, but was Activity")
            }
            PlanElement::Leg {
                mode,
                dep_time,
                trav_time,
                route,
            } => {
                // <leg mode=\"car\">
                //     <route type=\"links\" start_link=\"1\" end_link=\"20\" trav_time=\"undefined\" distance=\"25000.0\" vehicleRefId=\"null\">1 6 15 20</route>
                // </leg>
                assert_eq!("car", mode);
                assert_eq!(&None, trav_time);
                assert_eq!(&None, dep_time);
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
            PlanElement::Activity {
                end_time,
                start_time,
                x,
                y,
                max_dur,
                r#type,
                link,
            } => {
                //<activity type=\"w\" link=\"20\" x=\"10000.0\" y=\"0.0\" max_dur=\"03:30:00\" >
                assert_eq!("w", r#type);
                assert_eq!("20", link);
                assert_eq!(10000.0, *x);
                assert_eq!(0.0, *y);
                assert_eq!(&Some(String::from("03:30:00")), max_dur);
                assert_eq!(&None, start_time);
                assert_eq!(&None, end_time);
            }
            PlanElement::Leg { .. } => {
                panic!("Plan element at inded 6 was expected to be an activity but was a Leg.")
            }
        }
    }

    #[test]
    fn read_example_file() {
        let population = Population::from_file("./assets/population-v6-34-persons.xml");
        // println!("{population:#?}");

        assert_eq!(34, population.persons.len())
    }

    #[test]
    fn read_example_file_gzipped() {
        let population = Population::from_file("./assets/population-v6-34-persons.xml.gz");
        assert_eq!(34, population.persons.len())
    }
}
