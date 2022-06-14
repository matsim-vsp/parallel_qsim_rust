use serde::Deserialize;

use crate::container::xml_reader;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Route {
    pub r#type: String,
    pub start_link: String,
    pub end_link: String,
    pub trav_time: Option<String>,
    pub distance: f32,
    #[serde(rename = "vehicleRefId")]
    pub vehicle: Option<String>,

    // this needs to be parsed later
    #[serde(rename = "$value")]
    pub route: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Activity {
    pub r#type: String,
    pub link: String,
    pub x: f32,
    pub y: f32,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub max_dur: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Leg {
    pub mode: String,
    pub dep_time: Option<String>,
    pub trav_time: Option<String>,
    pub route: Route,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PlanElement {
    // the current matsim implementation has more logic with facility-id, link-id and coord.
    // This prototype assumes a fully specified activity with coord and link-id. We don't care about
    // Facilities at this stage.
    Activity(Activity),
    Leg(Leg),
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Plan {
    pub selected: bool,
    // https://users.rust-lang.org/t/serde-deserializing-a-vector-of-enums/51647/2
    #[serde(rename = "$value")]
    pub elements: Vec<PlanElement>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Person {
    pub id: String,
    #[serde(rename = "plan")]
    pub plans: Vec<Plan>,
}

impl Person {
    pub fn selected_plan(&self) -> &Plan {
        self.plans.iter().find(|p| p.selected).unwrap()
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Population {
    #[serde(rename = "person")]
    pub persons: Vec<Person>,
}

impl Population {
    pub fn from_file(file_path: &str) -> Population {
        xml_reader::read(file_path)
    }
}

#[cfg(test)]
mod tests {
    use quick_xml::de::from_str;

    use crate::container::population::{PlanElement, Population};

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
            PlanElement::Leg(leg) => {
                // <leg mode=\"car\">
                //     <route type=\"links\" start_link=\"1\" end_link=\"20\" trav_time=\"undefined\" distance=\"25000.0\" vehicleRefId=\"null\">1 6 15 20</route>
                // </leg>
                assert_eq!("car", leg.mode);
                assert_eq!(None, leg.trav_time);
                assert_eq!(None, leg.dep_time);
                assert_eq!("links", leg.route.r#type);
                assert_eq!("1", leg.route.start_link);
                assert_eq!("20", leg.route.end_link);
                assert_eq!("undefined", leg.route.trav_time.as_ref().unwrap());
                assert_eq!(25000.0, leg.route.distance);
                assert_eq!("null", leg.route.vehicle.as_ref().unwrap());
                assert_eq!("1 6 15 20", leg.route.route.as_ref().unwrap())
            }
        }

        let activity2 = plan.elements.get(4).unwrap();
        match activity2 {
            PlanElement::Activity(activity) => {
                //<activity type=\"w\" link=\"20\" x=\"10000.0\" y=\"0.0\" max_dur=\"03:30:00\" >
                assert_eq!("w", activity.r#type);
                assert_eq!("20", activity.link);
                assert_eq!(10000.0, activity.x);
                assert_eq!(0.0, activity.y);
                assert_eq!(Some(String::from("03:30:00")), activity.max_dur);
                assert_eq!(None, activity.start_time);
                assert_eq!(None, activity.end_time);
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
