use crate::simulation::config::Config;
use crate::simulation::id::Id;
use crate::simulation::network::global_network::Link;
use crate::simulation::population::{InternalPerson, InternalPlan};
use crate::simulation::scenario::Scenario;
use crate::simulation::vehicles::InternalVehicle;
use crate::simulation::InternalSimulationAgent;
use std::collections::HashMap;
use tracing::info;

pub trait AgentSource {
    fn create_agents(
        &self,
        scenario: &mut Scenario,
        config: &Config,
    ) -> HashMap<Id<InternalPerson>, InternalSimulationAgent>;
}

pub struct PopulationAgentSource {}

impl AgentSource for PopulationAgentSource {
    fn create_agents(
        &self,
        scenario: &mut Scenario,
        _config: &Config,
    ) -> HashMap<Id<InternalPerson>, InternalSimulationAgent> {
        // take Persons and copy them into queues. This way we can keep the population around to translate
        // ids for events processing...
        let persons = std::mem::take(&mut scenario.population.persons);
        let mut agents = HashMap::with_capacity(persons.len());

        for (id, person) in persons {
            Self::identify_logic_and_insert(&mut agents, id, person);
        }
        agents
    }
}

impl PopulationAgentSource {
    fn identify_logic_and_insert(
        agents: &mut HashMap<Id<InternalPerson>, InternalSimulationAgent>,
        id: Id<InternalPerson>,
        person: InternalPerson,
    ) {
        // go through all attributes of person's legs and check whether there is some marked as rolling horizon logic
        let has_at_least_one_rolling_horizon_planning = person
            .selected_plan()
            .as_ref()
            .unwrap_or_else(|| panic!("Plan does not exist for person with id: {}", id.external()))
            .legs()
            .iter()
            .any(|l| {
                l.attributes
                    .as_ref()
                    .map(|a| a.attributes.contains_key("rollingHorizonLogic"))
                    .unwrap_or_else(|| false)
            });

        if has_at_least_one_rolling_horizon_planning {
            agents.insert(id, InternalSimulationAgent::new(person));
        } else {
            // if there is no rolling horizon logic, we assume that the person has a plan logic
            // and we create a InternalSimulationAgent with plan logic
            agents.insert(id, InternalSimulationAgent::new(person));
        }
    }
}

pub struct DrtAgentSource {}

impl DrtAgentSource {
    fn add_drt_ids() {
        info!("Creating DRT ids.");

        //activity types
        Id::<String>::create("BeforeVrpSchedule");
        Id::<String>::create("DrtStay");
        Id::<String>::create("DrtBusStop");

        //task types
        Id::<String>::create("DRIVE");
        Id::<String>::create("STOP");
        Id::<String>::create("STAY");
    }

    fn add_drt_driver(
        scenario: &mut Scenario,
        config: &Config,
    ) -> HashMap<Id<InternalPerson>, InternalSimulationAgent> {
        info!("Creating DRT drivers");

        let drt_modes = config
            .drt()
            .as_ref()
            .unwrap()
            .services
            .iter()
            .map(|s| s.mode.clone())
            .collect::<Vec<String>>();

        //fetch all drt vehicles starting on this partition
        let local_drt_vehicles = scenario
            .garage
            .vehicles
            .values()
            .filter(|&v| {
                if let Some(value) = v.attributes.as_ref().unwrap().get::<String>("dvrpMode") {
                    drt_modes.contains(&value)
                } else {
                    false
                }
            })
            .map(|v| {
                let link = v
                    .attributes
                    .as_ref()
                    .unwrap()
                    .get::<String>("startLink")
                    .expect("No start link for drt vehicle provided.");
                let link_id = Id::<Link>::get_from_ext(link.as_str());
                (link_id, v)
            })
            .filter(|(l, _)| scenario.network_partition.get_link_ids().contains(l))
            .collect::<Vec<(_, &InternalVehicle)>>();

        let mut result = HashMap::new();

        //for each drt vehicle, create a driver agent
        for (link, vehicle) in local_drt_vehicles {
            let start = vehicle
                .attributes
                .as_ref()
                .unwrap()
                .get::<u32>("serviceBeginTime")
                .expect("No service begin time for drt vehicle provided.");

            let veh_id = vehicle.id.clone();
            let person_id = Id::<InternalPerson>::create(veh_id.external());
            let from = scenario.network_partition.links.get(&link).unwrap().from();
            let x = scenario.network.get_node(from).x;
            let y = scenario.network.get_node(from).y;

            let mut plan = InternalPlan::default();
            //TODO is Some(start) as end time correct?
            // plan.add_act(InternalActivity::new(
            //     x,
            //     y,
            //     "act",
            //     Id::new_internal(link),
            //     Some(0),
            //     Some(start),
            //     None,
            // ));

            let person = InternalPerson::new(person_id, plan);

            let agent_id = Id::<InternalPerson>::create(veh_id.external());
            //TODO
            result.insert(agent_id, InternalSimulationAgent::new(person));
        }
        result
    }
}

impl AgentSource for DrtAgentSource {
    fn create_agents(
        &self,
        scenario: &mut Scenario,
        config: &Config,
    ) -> HashMap<Id<InternalPerson>, InternalSimulationAgent> {
        Self::add_drt_ids();
        Self::add_drt_driver(scenario, config)
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::{CommandLineArgs, Config};
    use crate::simulation::id::Id;
    use crate::simulation::population::agent_source::{
        AgentSource, DrtAgentSource, PopulationAgentSource,
    };
    use crate::simulation::population::InternalPerson;
    use crate::simulation::scenario::Scenario;
    use crate::simulation::vehicles::InternalVehicle;
    use itertools::Itertools;
    use std::path::PathBuf;

    #[test]
    fn test_drt_agent_source() {
        let config_path = "./assets/drt/config.yml";
        let config = Config::from_file(&CommandLineArgs {
            config_path: String::from(config_path),
            num_parts: None,
        });

        let output_path = PathBuf::from(config.output().output_dir);

        let mut scenario = Scenario::build(&config, &String::from(config_path), 0, &output_path);

        let drt_source = DrtAgentSource {};
        let drt_agents = drt_source.create_agents(&mut scenario, &config);

        let agent_source = PopulationAgentSource {};
        let default_agents = agent_source.create_agents(&mut scenario, &config);

        assert_eq!(scenario.network.nodes().len(), 62);
        assert_eq!(scenario.network.links().len(), 170);

        // 10 agents, 1 drt agent
        assert_eq!(default_agents.len(), 10);
        assert_eq!(drt_agents.len(), 1);

        // 10 agent vehicles, 1 drt vehicle
        assert_eq!(scenario.garage.vehicles.len(), 10 + 1);

        //there is only one predefined vehicle type (car)
        assert_eq!(scenario.garage.vehicle_types.len(), 1);

        let default_agent_ids = default_agents.keys().collect::<Vec<&Id<InternalPerson>>>();

        let vehicle_ids = scenario
            .garage
            .vehicles
            .keys()
            .collect::<Vec<&Id<InternalVehicle>>>();

        for n in 0..10u64 {
            assert!(
                default_agent_ids.contains(&&Id::get_from_ext(format!("passenger{}", n).as_str()))
            );
            assert!(
                vehicle_ids.contains(&&Id::get_from_ext(format!("passenger{}_car", n).as_str()))
            );
        }

        assert!(drt_agents.keys().contains(&&Id::get_from_ext("drt")));
        assert!(vehicle_ids.contains(&&Id::get_from_ext("drt")));
    }
}
