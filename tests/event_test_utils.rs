use rust_q_sim::simulation::io::xml_reader;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::process::Command;
use std::str::FromStr;
use std::time::Duration;
use std::usize;
use wait_timeout::ChildExt;

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
    #[serde(rename = "travelled")]
    Travelled(Travelled),
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

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Travelled {
    #[serde(deserialize_with = "str_to_u64")]
    person: u64,
    #[serde(deserialize_with = "str_to_u64")]
    distance: u64,
    mode: String,
}

fn str_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    u64::from_str(&s).map_err(de::Error::custom)
}

pub fn run_mpi_simulation_and_convert_events(
    number_of_parts: usize,
    network_file: &str,
    population_file: &str,
    output_dir: &str,
    routing_mode: &str,
    vehicle_definitions_file: Option<&str>,
) {
    let mut command = Command::new("cargo");
    command
        .arg("mpirun")
        .arg("-n")
        .arg(format!("{}", number_of_parts))
        .arg("--bin")
        .arg("mpi_qsim")
        .arg("--")
        .arg("--network-file")
        .arg(network_file)
        .arg("--population-file")
        .arg(population_file)
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--routing-mode")
        .arg(routing_mode);

    if let Some(vehicle_definitions) = vehicle_definitions_file {
        command
            .arg("--vehicle-definitions-file")
            .arg(vehicle_definitions);
    }

    let mut child = command.spawn().unwrap();

    let simulation_status = match child.wait_timeout(Duration::from_secs(15)).unwrap() {
        None => {
            child.kill().unwrap();
            child.wait().unwrap().code()
        }
        Some(status) => status.code(),
    };

    assert_eq!(
        simulation_status,
        Some(0),
        "The Simulation did not finish and was killed."
    );

    let event_converison_status = Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("proto2xml")
        .arg("--")
        .arg("--num-parts")
        .arg(format!("{}", number_of_parts))
        .arg("--path")
        .arg(output_dir.to_owned() + "events")
        .status()
        .unwrap();

    assert_eq!(event_converison_status.code(), Some(0));
}

pub fn compare_events(output_dir: &str, path_to_expected_scenario_files: &str) {
    let mut expected_output_events: Events = xml_reader::read(
        (String::from(path_to_expected_scenario_files) + "/output_events.xml").as_ref(),
    );

    let actual_output_events: Events =
        xml_reader::read((output_dir.to_owned() + "/events.xml").as_ref());

    for actual_event in actual_output_events.events {
        expected_output_events.events.remove(
            expected_output_events
                .events
                .iter()
                .position(|expected_event| *expected_event == actual_event)
                .expect(
                    format!(
                        "Event {:?} was not expected to be in the output",
                        actual_event
                    )
                    .as_str(),
                ),
        );
    }

    assert!(
        expected_output_events.events.is_empty(),
        "The following events are missing in the actual output: {:#?}",
        expected_output_events.events
    );
}
