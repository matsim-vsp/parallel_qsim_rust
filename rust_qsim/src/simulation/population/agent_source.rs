use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::config::{Config, RoutingMode};
use crate::simulation::id::Id;
use crate::simulation::population::InternalPerson;
use crate::simulation::scenario::ScenarioPartition;
use std::collections::HashMap;

pub trait AgentSource {
    fn create_agents(
        &self,
        scenario: &mut ScenarioPartition,
    ) -> HashMap<Id<InternalPerson>, SimulationAgent>;
}

pub struct PopulationAgentSource {}

impl AgentSource for PopulationAgentSource {
    fn create_agents(
        &self,
        scenario: &mut ScenarioPartition,
    ) -> HashMap<Id<InternalPerson>, SimulationAgent> {
        // take Persons and copy them into queues. This way we can keep the population around to translate
        // ids for events processing...
        let persons = std::mem::take(&mut scenario.population.persons);
        let mut agents = HashMap::with_capacity(persons.len());

        for (id, person) in persons {
            Self::identify_logic_and_insert(&mut agents, id, person, &scenario.config);
        }
        agents
    }
}

impl PopulationAgentSource {
    fn identify_logic_and_insert(
        agents: &mut HashMap<Id<InternalPerson>, SimulationAgent>,
        id: Id<InternalPerson>,
        person: InternalPerson,
        config: &Config,
    ) {
        if config.routing().mode == RoutingMode::UsePlans {
            agents.insert(id, SimulationAgent::new_plan_based(person));
            return;
        }

        // go through all attributes of person's legs and check whether there is some marked as rolling horizon logic
        let has_at_least_one_preplanning_horizon = person
            .selected_plan()
            .as_ref()
            .unwrap_or_else(|| panic!("Plan does not exist for person with id: {}", id.external()))
            .acts()
            .iter()
            .any(|l| {
                l.attributes
                    .attributes
                    .contains_key(crate::simulation::population::PREPLANNING_HORIZON)
            });

        if has_at_least_one_preplanning_horizon {
            agents.insert(id, SimulationAgent::new_adaptive_plan_based(person));
        } else {
            // if there is no rolling horizon logic, we assume that the person has a plan logic
            // and we create a InternalSimulationAgent with plan logic
            agents.insert(id, SimulationAgent::new_plan_based(person));
        }
    }
}
