use crate::simulation::config::Config;
use crate::simulation::id::Id;
use crate::simulation::population::InternalPerson;
use crate::simulation::scenario::Scenario;
use crate::simulation::InternalSimulationAgent;
use std::collections::HashMap;

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
            .any(|l| l.attributes.attributes.contains_key("rollingHorizonLogic"));

        if has_at_least_one_rolling_horizon_planning {
            agents.insert(id, InternalSimulationAgent::new(person));
        } else {
            // if there is no rolling horizon logic, we assume that the person has a plan logic
            // and we create a InternalSimulationAgent with plan logic
            agents.insert(id, InternalSimulationAgent::new(person));
        }
    }
}
