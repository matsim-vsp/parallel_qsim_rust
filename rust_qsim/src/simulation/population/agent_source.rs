use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::config::{Config, RoutingMode};
use crate::simulation::id::Id;
use crate::simulation::scenario::ScenarioPartition;
use crate::simulation::scenario::population::InternalPerson;
use std::collections::HashMap;
use std::sync::Arc;

// needs to be an Arc since agents are inserted on every partition.
pub type DynAgentSource = Arc<dyn AgentSource + Send + Sync>;

pub trait AgentSource {
    fn create_agents(
        &self,
        scenario: &mut ScenarioPartition,
    ) -> HashMap<Id<InternalPerson>, SimulationAgent>;
}

pub trait IntoDynAgentSource {
    fn into_dyn_agent_source(self) -> DynAgentSource;
}

impl<T> IntoDynAgentSource for T
where
    T: AgentSource + Send + Sync + 'static,
{
    fn into_dyn_agent_source(self) -> DynAgentSource {
        Arc::new(self)
    }
}

impl<T> IntoDynAgentSource for Arc<T>
where
    T: AgentSource + Send + Sync + 'static,
{
    fn into_dyn_agent_source(self) -> DynAgentSource {
        self
    }
}

impl IntoDynAgentSource for DynAgentSource {
    fn into_dyn_agent_source(self) -> DynAgentSource {
        self
    }
}

pub struct PopulationAgentSource;

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
            agents.insert(id, SimulationAgent::new_plan_based(person));
        }
        agents
    }
}

pub struct PreplanningHorizonAgentSource;

impl AgentSource for PreplanningHorizonAgentSource {
    fn create_agents(
        &self,
        scenario: &mut ScenarioPartition,
    ) -> HashMap<Id<InternalPerson>, SimulationAgent> {
        // take Persons and copy them into queues. This way we can keep the population around to translate
        // ids for events processing...
        let persons = std::mem::take(&mut scenario.population.persons);
        let mut agents = HashMap::with_capacity(persons.len());

        for (id, person) in persons {
            identify_logic_and_insert(&mut agents, id, person, &scenario.config);
        }
        agents
    }
}

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
                .contains_key(crate::simulation::scenario::population::PREPLANNING_HORIZON)
        });

    if has_at_least_one_preplanning_horizon {
        agents.insert(id, SimulationAgent::new_adaptive_plan_based(person));
    } else {
        // if there is no rolling horizon logic, we assume that the person has a plan logic
        // and we create a InternalSimulationAgent with plan logic
        agents.insert(id, SimulationAgent::new_plan_based(person));
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentSource, DynAgentSource, IntoDynAgentSource};
    use crate::simulation::agents::agent::SimulationAgent;
    use crate::simulation::id::Id;
    use crate::simulation::scenario::ScenarioPartition;
    use crate::simulation::scenario::population::InternalPerson;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct TestAgentSource;

    impl AgentSource for TestAgentSource {
        fn create_agents(
            &self,
            _scenario: &mut ScenarioPartition,
        ) -> HashMap<Id<InternalPerson>, SimulationAgent> {
            HashMap::new()
        }
    }

    #[test]
    fn converts_concrete_agent_source_to_dyn_agent_source() {
        let source = TestAgentSource.into_dyn_agent_source();

        assert_eq!(Arc::strong_count(&source), 1);
    }

    #[test]
    fn converts_arc_agent_source_to_dyn_agent_source() {
        let source = Arc::new(TestAgentSource);
        let dyn_source = source.clone().into_dyn_agent_source();

        assert_eq!(Arc::strong_count(&source), 2);
        assert_eq!(Arc::strong_count(&dyn_source), 2);
    }

    #[test]
    fn accepts_existing_dyn_agent_source() {
        let source: DynAgentSource = Arc::new(TestAgentSource);
        let dyn_source = source.clone().into_dyn_agent_source();

        assert_eq!(Arc::strong_count(&source), 2);
        assert_eq!(Arc::strong_count(&dyn_source), 2);
    }
}
