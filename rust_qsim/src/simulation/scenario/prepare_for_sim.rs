use crate::simulation::config::Config;
use crate::simulation::scenario::network::Network;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::Garage;
use crate::simulation::scenario::{Coordinate, MutableScenario};
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
    context: &PrepareForSimContext<'_>,
    person: &mut InternalPerson,
) -> Vec<PrepareForSimIssue> {
    person
        .plans_mut()
        .iter_mut()
        .flat_map(|p| p.acts_mut())
        .for_each(|a| {
            if a.coord.is_none() {
                let link = context.network.get_link(&a.link_id);
                let from = context.network.get_node(&link.from);
                let to = context.network.get_node(&link.to);

                let coord = Coordinate::middle(&from.coord, &to.coord);

                a.coord = Some(coord);
            }
        });

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::prepare_for_sim;
    use crate::simulation::config::Config;
    use crate::simulation::id::Id;
    use crate::simulation::scenario::network::{Link, Network, Node};
    use crate::simulation::scenario::population::{
        InternalActivity, InternalPerson, InternalPlan, Population,
    };
    use crate::simulation::scenario::vehicles::Garage;
    use crate::simulation::scenario::{Coordinate, MutableScenario};
    use macros::integration_test;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[integration_test]
    fn prepare_for_sim_succeeds_for_empty_population() {
        let mut scenario = scenario_with_population(Population::new());

        prepare_for_sim(&mut scenario).unwrap();

        assert!(scenario.population.persons.is_empty());
    }

    #[integration_test]
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

    #[integration_test]
    fn prepare_for_sim_assigns_missing_activity_coordinates() {
        let person_id = Id::create("person-1");
        let link_id = Id::create("link-1");
        let mut plan = InternalPlan::default();
        plan.add_act(InternalActivity::new(
            None,
            "act",
            link_id.clone(),
            None,
            None,
            None,
        ));

        let mut persons = HashMap::new();
        persons.insert(
            person_id.clone(),
            InternalPerson::new(person_id.clone(), plan),
        );
        let mut scenario = scenario_with_network_and_population(
            network_with_link(link_id),
            Population { persons },
        );

        prepare_for_sim(&mut scenario).unwrap();

        let person = scenario.population.persons.get(&person_id).unwrap();
        let act = person.selected_plan().unwrap().acts()[0];
        assert_eq!(
            Some(&Coordinate::new_3d(5.0, 15.0, 10.0)),
            act.coord.as_ref()
        );
    }

    fn scenario_with_population(population: Population) -> MutableScenario {
        scenario_with_network_and_population(Network::new(), population)
    }

    fn scenario_with_network_and_population(
        network: Network,
        population: Population,
    ) -> MutableScenario {
        MutableScenario {
            network,
            garage: Garage::default(),
            population,
            config: Arc::new(Config::default()),
        }
    }

    fn network_with_link(link_id: Id<Link>) -> Network {
        let mut network = Network::new();
        let from = Node::new(
            Id::create("from-node"),
            Coordinate::new_3d(0.0, 10.0, 4.0),
            0,
            1,
        );
        let to = Node::new(
            Id::create("to-node"),
            Coordinate::new_3d(10.0, 20.0, 16.0),
            0,
            1,
        );
        let link = Link::new_with_default(link_id, &from, &to);

        network.add_node(from);
        network.add_node(to);
        network.add_link(link);
        network
    }

    fn person(id: &str, link_id: &str) -> InternalPerson {
        let mut plan = InternalPlan::default();
        plan.add_act(InternalActivity::new(
            Some(Coordinate::default()),
            "act",
            Id::create(link_id),
            None,
            None,
            None,
        ));
        InternalPerson::new(Id::create(id), plan)
    }
}
