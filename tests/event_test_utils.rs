use rust_q_sim::config::Config;
use rust_q_sim::logging::init_logging;
use rust_q_sim::{controller, io};
use serde::{Deserialize, Serialize};

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
    time: String,
    person: String,
    link: String,
    #[serde(rename = "actType")]
    act_type: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct ArrivalDeparture {
    time: String,
    person: String,
    link: String,
    #[serde(rename = "legMode")]
    leg_mode: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct PersonVehicleInteraction {
    time: String,
    person: String,
    vehicle: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct LinkInteraction {
    time: String,
    link: String,
    vehicle: String,
}

pub fn run_simulation_and_compare_events(config: Config, path_to_expected_scenario_files: &str) {
    let output_dir = config.output_dir.clone();
    let _logger_guard = init_logging(&output_dir);

    controller::run(config);

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
