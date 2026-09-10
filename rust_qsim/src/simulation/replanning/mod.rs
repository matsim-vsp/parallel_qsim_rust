use crate::simulation::config;
use crate::simulation::id::Id;
use crate::simulation::random::get_rng;
use crate::simulation::scenario::population::{DEFAULT_SUBPOPULATION, InternalPerson, Population};
use derive_builder::Builder;
use nohash_hasher::IntMap;
use rand::RngExt;
use rayon::prelude::*;
use selectors::{DefaultSelector, KeepLastSelector, WorstScoreSelector};
use std::fmt;
use std::str::FromStr;

pub mod routing;
mod selectors;

const STRATEGY_RNG_PURPOSE: &str = "replanning.strategy";
const RANDOM_SELECTOR_RNG_PURPOSE: &str = "replanning.selector.random";
pub const KEEP_LAST_SELECTED_STRATEGY_NAME: &str = "KeepLastSelected";
pub const BEST_SCORE_STRATEGY_NAME: &str = "BestScore";
pub const SELECT_RANDOM_STRATEGY_NAME: &str = "SelectRandom";
pub const WORST_SCORE_STRATEGY_NAME: &str = "WorstScore";
pub const RE_ROUTE_STRATEGY_NAME: &str = "ReRoute";

#[allow(dead_code)]
/// This is responsible for picking a plan, copying it, and replanning it.
trait PlanStrategy: Send + Sync {
    fn name(&self) -> &Id<String>;
    fn handle(&self, person: &mut InternalPerson, context: &ReplanningContext);
}

#[allow(dead_code)]
/// This is the smallest replanning unit (e.g., routes a plan).
trait PlanStrategyModule: Send + Sync {
    fn handle(&self, person: &mut InternalPerson, plan_index: usize);
}

/// This is responsible for selecting a plan from a person's available plans.
trait PlanSelector: Send + Sync {
    fn select(&self, person: &InternalPerson, context: &ReplanningContext) -> usize;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefaultStrategy {
    ReRoute,
}

impl DefaultStrategy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReRoute => RE_ROUTE_STRATEGY_NAME,
        }
    }

    fn as_generic_plan_strategy(self) -> Box<dyn PlanStrategy> {
        match self {
            Self::ReRoute => Box::new(GenericPlanStrategy {
                name: Id::create(self.as_str()),
                selector: Box::new(KeepLastSelector),
                modules: vec![Box::new(ReRouteModule {})],
            }),
        }
    }
}

impl fmt::Display for DefaultStrategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for DefaultStrategy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            RE_ROUTE_STRATEGY_NAME => Ok(Self::ReRoute),
            _ => Err(format!("Unknown DefaultStrategy: {value}")),
        }
    }
}

/// Performs multithreaded replanning of the population
pub(crate) fn replan_population(
    population: Population,
    iteration: u32,
    base_seed: u64,
    strategy_manager: &StrategyManager,
    innovation_disabled: bool,
) -> Population {
    let persons = population
        .persons
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(id, mut person)| {
            strategy_manager.run(iteration, base_seed, innovation_disabled, &mut person);
            (id, person)
        })
        .collect();

    Population { persons }
}

#[allow(dead_code)]
#[derive(Builder)]
#[builder(pattern = "owned")]
/// Manages replanning. This is the registry for all the replanning strategies.
pub(crate) struct StrategyManager {
    #[builder(default = "default_weights_per_subpopulation()")]
    weights_per_subpopulation: IntMap<Id<String>, StrategyWeights>,
    #[builder(default = "default_max_memory_size()")]
    max_memory_size: usize,
    #[builder(default = "default_plan_remover()")]
    plan_remover: Box<dyn PlanSelector>,
    #[builder(default = "default_strategies()")]
    strategies: IntMap<Id<String>, Box<dyn PlanStrategy>>,
}

impl StrategyManager {
    pub(crate) fn from_replanning_config(replanning: &config::Replanning) -> Self {
        let weights_per_subpopulation =
            weights_per_subpopulation_from_settings(&replanning.strategy_settings);

        StrategyManagerBuilder::default()
            .weights_per_subpopulation(weights_per_subpopulation)
            .max_memory_size(replanning.max_agent_plan_memory as usize)
            .plan_remover(plan_selector_from_config_name(
                &replanning.plan_selector_for_removal,
            ))
            .strategies(default_strategies())
            .build()
            .unwrap()
    }

    fn run(
        &self,
        iteration: u32,
        base_seed: u64,
        innovation_disabled: bool,
        person: &mut InternalPerson,
    ) {
        let context = ReplanningContext {
            iteration,
            base_seed,
            innovation_disabled,
        };

        if let Some(strategy) = self.choose_strategy(&context, person) {
            strategy.handle(person, &context);
        }
        self.remove_plans_if_needed(person, &context);
    }

    /// Chooses a strategy and runs it.
    fn choose_strategy(
        &self,
        context: &ReplanningContext,
        person: &InternalPerson,
    ) -> Option<&dyn PlanStrategy> {
        let weights = self.weights_per_subpopulation.get(person.subpopulation())?;
        let allowed_entries = weights
            .entries
            .iter()
            .filter(|entry| entry.weight > 0.0)
            .filter(|entry| {
                !context.innovation_disabled || is_non_innovative_strategy(&entry.strategy_name)
            })
            .collect::<Vec<_>>();

        let total_weight: f64 = allowed_entries.iter().map(|entry| entry.weight).sum();
        if total_weight <= 0.0 {
            return None;
        }

        let stream_id = format!("{}:{}", context.iteration, person.id().external());
        let mut rng = get_rng(context.base_seed, STRATEGY_RNG_PURPOSE, &stream_id);

        // Weighted random selection over positive strategy weights.
        let mut draw = rng.random_range(0.0..total_weight);
        for entry in &allowed_entries {
            if draw < entry.weight {
                return Some(self.strategy_by_name(&entry.strategy_name));
            }
            draw -= entry.weight;
        }

        allowed_entries
            .into_iter()
            .rev()
            .map(|entry| self.strategy_by_name(&entry.strategy_name))
            .next()
    }

    fn remove_plans_if_needed(&self, person: &mut InternalPerson, context: &ReplanningContext) {
        while person.plans().len() > self.max_memory_size {
            let index = self.plan_remover.select(person, context);
            person.plans_mut().remove(index);
        }
    }

    fn strategy_by_name(&self, strategy_name: &Id<String>) -> &dyn PlanStrategy {
        self.strategies
            .get(strategy_name)
            .map(Box::as_ref)
            .unwrap_or_else(|| panic!("No replanning strategy registered for {strategy_name}"))
    }
}

impl Default for StrategyManager {
    fn default() -> Self {
        StrategyManagerBuilder::default().build().unwrap()
    }
}

fn default_weights_per_subpopulation() -> IntMap<Id<String>, StrategyWeights> {
    let mut weights_per_subpopulation = IntMap::default();
    weights_per_subpopulation.insert(
        Id::create(DEFAULT_SUBPOPULATION),
        StrategyWeights::new(vec![StrategyWeight::new(
            Id::create(DefaultSelector::KeepLastSelected.as_str()),
            1.0,
        )]),
    );
    weights_per_subpopulation
}

fn default_max_memory_size() -> usize {
    5
}

fn default_plan_remover() -> Box<dyn PlanSelector> {
    Box::new(WorstScoreSelector)
}

fn default_strategies() -> IntMap<Id<String>, Box<dyn PlanStrategy>> {
    let mut strategies = IntMap::default();
    for selector in [
        DefaultSelector::KeepLastSelected,
        DefaultSelector::BestScore,
        DefaultSelector::SelectRandom,
        DefaultSelector::WorstScore,
    ] {
        strategies.insert(
            Id::create(selector.as_str()),
            selector.as_generic_plan_strategy(),
        );
    }
    for strategy in [DefaultStrategy::ReRoute] {
        strategies.insert(
            Id::create(strategy.as_str()),
            strategy.as_generic_plan_strategy(),
        );
    }
    strategies
}

fn weights_per_subpopulation_from_settings(
    settings: &[config::StrategySetting],
) -> IntMap<Id<String>, StrategyWeights> {
    let mut weights_per_subpopulation = IntMap::default();
    for setting in settings {
        assert_known_strategy_name(&setting.name);
        weights_per_subpopulation
            .entry(Id::create(&setting.subpopulation))
            .or_insert_with(|| StrategyWeights::new(Vec::new()))
            .entries
            .push(StrategyWeight::new(
                Id::create(&setting.name),
                setting.weight,
            ));
    }
    weights_per_subpopulation
}

fn assert_known_strategy_name(name: &str) {
    if DefaultSelector::from_str(name).is_ok() || DefaultStrategy::from_str(name).is_ok() {
        return;
    }
    panic!("Unknown replanning strategy or selector configured: {name}");
}

fn plan_selector_from_config_name(name: &str) -> Box<dyn PlanSelector> {
    DefaultSelector::from_str(name)
        .map(DefaultSelector::as_plan_selector)
        .unwrap_or_else(|_| panic!("Unknown plan_selector_for_removal configured: {name}"))
}

fn is_non_innovative_strategy(strategy_name: &Id<String>) -> bool {
    DefaultSelector::from_str(strategy_name.external()).is_ok()
}

struct StrategyWeights {
    entries: Vec<StrategyWeight>,
}

impl StrategyWeights {
    fn new(entries: Vec<StrategyWeight>) -> Self {
        Self { entries }
    }
}

struct StrategyWeight {
    strategy_name: Id<String>,
    weight: f64,
}

impl StrategyWeight {
    fn new(strategy_name: Id<String>, weight: f64) -> Self {
        Self {
            strategy_name,
            weight,
        }
    }
}

// Different modules can be combined to create a strategy.
struct GenericPlanStrategy {
    name: Id<String>,
    selector: Box<dyn PlanSelector + Send + Sync>,
    modules: Vec<Box<dyn PlanStrategyModule + Send + Sync>>,
}

impl PlanStrategy for GenericPlanStrategy {
    fn name(&self) -> &Id<String> {
        &self.name
    }

    fn handle(&self, person: &mut InternalPerson, context: &ReplanningContext) {
        let plan_index = self.selector.select(person, context);
        if self.modules.is_empty() {
            return;
        }
        let mut new_plan = person
            .plans()
            .get(plan_index)
            .cloned()
            .unwrap_or_else(|| panic!("Selected plan index {plan_index} does not exist."));
        for plan in person.plans_mut() {
            plan.selected = false;
        }
        new_plan.selected = true;
        person.plans_mut().push(new_plan);
        let new_plan_index = person.plans().len() - 1;

        for module in &self.modules {
            module.handle(person, new_plan_index);
        }
    }
}

#[allow(dead_code)]
struct ReRouteModule {
    // hold reference to scenario
    // hold reference to router
}

impl PlanStrategyModule for ReRouteModule {
    fn handle(&self, _person: &mut InternalPerson, _plan_index: usize) {
        unimplemented!("ReRouteModule is a placeholder and does not implement routing yet.")
    }
}

struct ReplanningContext {
    iteration: u32,
    base_seed: u64,
    innovation_disabled: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        GenericPlanStrategy, PlanStrategy, PlanStrategyModule, ReplanningContext, StrategyManager,
    };
    use crate::simulation::config::{Replanning, StrategySetting};
    use crate::simulation::id::Id;
    use crate::simulation::replanning::selectors::{DefaultSelector, KeepLastSelector};
    use crate::simulation::scenario::population::{InternalPerson, InternalPlan};
    use macros::deterministic_id_test;

    #[deterministic_id_test]
    fn default_selectors_create_generic_strategies_with_matching_names() {
        for selector in [
            DefaultSelector::KeepLastSelected,
            DefaultSelector::BestScore,
            DefaultSelector::SelectRandom,
        ] {
            let strategy = selector.as_generic_plan_strategy();

            assert_eq!(&Id::create(selector.as_str()), strategy.name());
        }
    }

    #[deterministic_id_test]
    fn config_manager_uses_memory_limit_and_removal_selector() {
        let replanning = Replanning {
            max_agent_plan_memory: 1,
            plan_selector_for_removal: DefaultSelector::BestScore.as_str().to_string(),
            ..Replanning::default()
        };
        let manager = StrategyManager::from_replanning_config(&replanning);
        let mut person = person_with_scores([Some(1.0), Some(2.0)]);

        manager.run(0, 42, false, &mut person);

        assert_eq!(1, person.plans().len());
        assert_eq!(Some(1.0), person.plans()[0].score);
    }

    #[deterministic_id_test]
    fn config_manager_groups_strategy_weights_by_subpopulation() {
        let replanning = Replanning {
            strategy_settings: vec![
                StrategySetting {
                    name: DefaultSelector::KeepLastSelected.as_str().to_string(),
                    weight: 0.3,
                    subpopulation: "person".to_string(),
                },
                StrategySetting {
                    name: DefaultSelector::BestScore.as_str().to_string(),
                    weight: 0.7,
                    subpopulation: "freight".to_string(),
                },
            ],
            ..Replanning::default()
        };
        let manager = StrategyManager::from_replanning_config(&replanning);

        let person_weights = manager
            .weights_per_subpopulation
            .get(&Id::create("person"))
            .unwrap();
        let freight_weights = manager
            .weights_per_subpopulation
            .get(&Id::create("freight"))
            .unwrap();

        assert_eq!(1, person_weights.entries.len());
        assert_eq!(0.3, person_weights.entries[0].weight);
        assert_eq!(1, freight_weights.entries.len());
        assert_eq!(0.7, freight_weights.entries[0].weight);
    }

    #[deterministic_id_test]
    fn innovation_filter_keeps_selectors_and_excludes_default_strategies() {
        let replanning = Replanning {
            strategy_settings: vec![
                StrategySetting {
                    name: "ReRoute".to_string(),
                    weight: 1.0,
                    subpopulation: "person".to_string(),
                },
                StrategySetting {
                    name: DefaultSelector::BestScore.as_str().to_string(),
                    weight: 1.0,
                    subpopulation: "person".to_string(),
                },
            ],
            ..Replanning::default()
        };
        let manager = StrategyManager::from_replanning_config(&replanning);
        let person = person_with_scores([Some(1.0), Some(2.0)]);
        let context = ReplanningContext {
            innovation_disabled: true,
            ..context()
        };

        let strategy = manager.choose_strategy(&context, &person).unwrap();

        assert_eq!(
            &Id::create(DefaultSelector::BestScore.as_str()),
            strategy.name()
        );
    }

    #[deterministic_id_test]
    fn default_strategy_is_named_generic_keep_last_selected() {
        let manager = StrategyManager::default();
        let strategy_name = Id::create(DefaultSelector::KeepLastSelected.as_str());
        let strategy = manager.strategies.get(&strategy_name).unwrap();
        let default_weight = &manager
            .weights_per_subpopulation
            .get(&Id::create("person"))
            .unwrap()
            .entries[0];

        assert_eq!(&strategy_name, strategy.name());
        assert_eq!(strategy.name(), &default_weight.strategy_name);
    }

    #[deterministic_id_test]
    fn generic_strategy_without_modules_does_not_copy_plan() {
        let strategy = GenericPlanStrategy {
            name: Id::create("KeepLastSelected"),
            selector: Box::new(KeepLastSelector),
            modules: Vec::new(),
        };
        let mut person = person_with_scores([Some(1.0)]);

        strategy.handle(&mut person, &context());

        assert_eq!(1, person.plans().len());
        assert!(person.plans()[0].selected);
        assert_eq!(Some(1.0), person.plans()[0].score);
    }

    #[deterministic_id_test]
    fn generic_strategy_with_modules_copies_plan_and_runs_modules_on_copy() {
        let strategy = GenericPlanStrategy {
            name: Id::create("ReRoute"),
            selector: Box::new(KeepLastSelector),
            modules: vec![Box::new(MarkCopiedPlanModule)],
        };
        let mut person = person_with_scores([Some(1.0)]);

        strategy.handle(&mut person, &context());

        assert_eq!(2, person.plans().len());
        assert!(!person.plans()[0].selected);
        assert_eq!(Some(1.0), person.plans()[0].score);
        assert!(person.plans()[1].selected);
        assert_eq!(Some(99.0), person.plans()[1].score);
    }

    pub fn person_with_scores<const N: usize>(scores: [Option<f64>; N]) -> InternalPerson {
        let mut person = InternalPerson::new(Id::create("person"), plan(scores[0], true));
        for score in scores.into_iter().skip(1) {
            person.plans_mut().push(plan(score, false));
        }
        person
    }

    fn plan(score: Option<f64>, selected: bool) -> InternalPlan {
        InternalPlan {
            score,
            selected,
            elements: Vec::new(),
        }
    }

    pub fn context() -> ReplanningContext {
        ReplanningContext {
            iteration: 7,
            base_seed: 42,
            innovation_disabled: false,
        }
    }

    struct MarkCopiedPlanModule;

    impl PlanStrategyModule for MarkCopiedPlanModule {
        fn handle(&self, person: &mut InternalPerson, plan_index: usize) {
            person.plans_mut()[plan_index].score = Some(99.0);
        }
    }
}
