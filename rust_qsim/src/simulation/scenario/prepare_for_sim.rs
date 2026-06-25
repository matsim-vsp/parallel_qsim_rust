use crate::simulation::config::Config;
use crate::simulation::scenario::MutableScenario;
use crate::simulation::scenario::network::Network;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::Garage;
use rayon::prelude::*;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("prepare-for-sim failed")]
pub struct PrepareForSimError {
    issues: Vec<PrepareForSimIssue>,
}

impl PrepareForSimError {
    fn new(mut issues: Vec<PrepareForSimIssue>) -> Self {
        issues.sort_by(|a, b| a.person_id.cmp(&b.person_id));
        Self { issues }
    }

    pub fn issues(&self) -> &[PrepareForSimIssue] {
        &self.issues
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareForSimIssue {
    pub person_id: String,
    pub message: String,
}

pub fn prepare_for_sim(scenario: &mut MutableScenario) -> Result<(), PrepareForSimError> {
    let context = PrepareForSimContext {
        network: &scenario.network,
        garage: &scenario.garage,
        config: scenario.config.as_ref(),
    };

    let issues: Vec<_> = scenario
        .population
        .persons
        .par_iter_mut()
        .flat_map(|(_, person)| prepare_person(&context, person))
        .collect();

    if issues.is_empty() {
        Ok(())
    } else {
        Err(PrepareForSimError::new(issues))
    }
}

pub struct PrepareForSimContext<'a> {
    pub network: &'a Network,
    pub garage: &'a Garage,
    pub config: &'a Config,
}

fn prepare_person(
    _context: &PrepareForSimContext<'_>,
    _person: &mut InternalPerson,
) -> Vec<PrepareForSimIssue> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::prepare_for_sim;
    use crate::simulation::config::Config;
    use crate::simulation::id::Id;
    use crate::simulation::scenario::network::Network;
    use crate::simulation::scenario::population::{
        InternalActivity, InternalPerson, InternalPlan, Population,
    };
    use crate::simulation::scenario::vehicles::Garage;
    use crate::simulation::scenario::{Coordinate, MutableScenario};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn prepare_for_sim_succeeds_for_empty_population() {
        let mut scenario = scenario_with_population(Population::new());

        prepare_for_sim(&mut scenario).unwrap();

        assert!(scenario.population.persons.is_empty());
    }

    #[test]
    fn prepare_for_sim_visits_population_without_moving_persons() {
        let mut persons = HashMap::new();
        persons.insert(Id::create("person-1"), person("person-1", "link-1"));
        persons.insert(Id::create("person-2"), person("person-2", "link-2"));
        let mut scenario = scenario_with_population(Population { persons });

        prepare_for_sim(&mut scenario).unwrap();

        assert_eq!(2, scenario.population.persons.len());
        assert!(
            scenario
                .population
                .persons
                .contains_key(&Id::get_from_ext("person-1"))
        );
        assert!(
            scenario
                .population
                .persons
                .contains_key(&Id::get_from_ext("person-2"))
        );
    }

    fn scenario_with_population(population: Population) -> MutableScenario {
        MutableScenario {
            network: Network::new(),
            garage: Garage::default(),
            population,
            config: Arc::new(Config::default()),
        }
    }

    fn person(id: &str, link_id: &str) -> InternalPerson {
        let mut plan = InternalPlan::default();
        plan.add_act(InternalActivity::new(
            Coordinate::default(),
            "act",
            Id::create(link_id),
            None,
            None,
            None,
        ));
        InternalPerson::new(Id::create(id), plan)
    }
}
