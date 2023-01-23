use rust_q_sim::config::Config;
use rust_q_sim::logging::init_logging;
use rust_q_sim::{controller, io};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::str::FromStr;
use std::time::Duration;
use std::usize;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Events {
    #[serde(rename = "event", default)]
    events: Vec<SimEvent>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(tag = "type")]
enum SimEvent {
    #[serde(rename = "actend")]
    ActivityEnd(Activity),
    #[serde(rename = "departure")]
    Departure(ArrivalDeparture),
    #[serde(rename = "PersonEntersVehicle")]
    PersonEntersVehicle(PersonVehicleInteraction),
    #[serde(rename = "left link")]
    LeftLink(LinkInteraction),
    #[serde(rename = "entered link")]
    EnteredLink(LinkInteraction),
    #[serde(rename = "PersonLeavesVehicle")]
    PersonLeavesVehicle(PersonVehicleInteraction),
    #[serde(rename = "arrival")]
    Arrival(ArrivalDeparture),
    #[serde(rename = "actstart")]
    ActivityStart(Activity),
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Activity {
    #[serde(deserialize_with = "str_to_u64")]
    time: u64,
    #[serde(deserialize_with = "str_to_u64")]
    person: u64,
    #[serde(deserialize_with = "str_to_u64")]
    link: u64,
    #[serde(rename = "actType")]
    act_type: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct ArrivalDeparture {
    #[serde(deserialize_with = "str_to_u64")]
    time: u64,
    #[serde(deserialize_with = "str_to_u64")]
    person: u64,
    #[serde(deserialize_with = "str_to_u64")]
    link: u64,
    #[serde(rename = "legMode")]
    leg_mode: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct PersonVehicleInteraction {
    #[serde(deserialize_with = "str_to_u64")]
    time: u64,
    #[serde(deserialize_with = "str_to_u64")]
    person: u64,
    #[serde(deserialize_with = "str_to_u64")]
    vehicle: u64,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct LinkInteraction {
    #[serde(deserialize_with = "str_to_u64")]
    time: u64,
    #[serde(deserialize_with = "str_to_u64")]
    link: u64,
    #[serde(deserialize_with = "str_to_u64")]
    vehicle: u64,
}

fn str_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    u64::from_str(&s).map_err(de::Error::custom)
}

pub fn run_simulation_and_compare_events(config: Config, path_to_expected_scenario_files: &str) {
    let output_dir = config.output_dir.clone();
    let _logger_guard = init_logging(&output_dir);

    controller::run(config);

    std::thread::sleep(Duration::from_secs(3));

    let mut expected_output_events: Events = io::xml_reader::read(
        (String::from(path_to_expected_scenario_files) + "/output_events.xml").as_ref(),
    );

    let actual_output_events: Events =
        io::xml_reader::read((output_dir + "/output_events.xml").as_ref());

    for actual_event in actual_output_events.events {
        expected_output_events.events.remove(
            expected_output_events
                .events
                .iter()
                .position(|expected_event| *expected_event == actual_event)
                .expect(&*format!(
                    "Event {:?} was not expected to be in the output",
                    actual_event
                )),
        );
    }

    assert!(
        expected_output_events.events.is_empty(),
        "The following events are missing in the actual output: {:#?}",
        expected_output_events.events
    );
}
