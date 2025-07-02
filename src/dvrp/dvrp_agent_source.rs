use crate::simulation::config::Config;
use crate::simulation::id::Id;
use crate::simulation::network::Link;
use crate::simulation::population::agent_source::AgentSource;
use crate::simulation::population::{InternalPerson, InternalPlan};
use crate::simulation::scenario::Scenario;
use crate::simulation::vehicles::InternalVehicle;
use crate::simulation::InternalSimulationAgent;
use std::collections::HashMap;
use tracing::info;

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
                if let Some(value) = v.attributes.get::<String>("dvrpMode") {
                    drt_modes.contains(&value)
                } else {
                    false
                }
            })
            .map(|v| {
                let link = v
                    .attributes
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
            let _start = vehicle
                .attributes
                .get::<u32>("serviceBeginTime")
                .expect("No service begin time for drt vehicle provided.");

            let veh_id = vehicle.id.clone();
            let person_id = Id::<InternalPerson>::create(veh_id.external());
            let from = scenario.network_partition.links.get(&link).unwrap().from();
            let _x = scenario.network.get_node(from).x;
            let _y = scenario.network.get_node(from).y;

            let plan = InternalPlan::default();
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
    use crate::dvrp::dvrp_agent_source::DrtAgentSource;
    use crate::simulation::config::{CommandLineArgs, Config};
    use crate::simulation::id::Id;
    use crate::simulation::population::agent_source::{AgentSource, PopulationAgentSource};
    use crate::simulation::population::InternalPerson;
    use crate::simulation::scenario::Scenario;
    use crate::simulation::vehicles::InternalVehicle;
    use itertools::Itertools;
    use std::path::PathBuf;

    #[test]
    #[ignore]
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
