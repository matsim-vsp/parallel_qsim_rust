use crate::simulation::id::Id;
use crate::simulation::random::get_rng;
use crate::simulation::scenario::population::{DEFAULT_SUBPOPULATION, InternalPerson, Population};
use ahash::HashMap;
use derive_builder::Builder;
use rand::Rng;
use rayon::prelude::*;
use std::fmt;
use std::str::FromStr;

pub mod routing;

const STRATEGY_RNG_PURPOSE: &str = "replanning.strategy";
const RANDOM_SELECTOR_RNG_PURPOSE: &str = "replanning.selector.random";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultSelector {
    KeepLastSelected,
    BestScore,
    SelectRandom,
}

impl DefaultSelector {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::KeepLastSelected => "KeepLastSelected",
            Self::BestScore => "BestScore",
            Self::SelectRandom => "SelectRandom",
        }
    }

    pub(crate) fn generic_plan_strategy(self) -> Box<dyn PlanStrategy> {
        let name = Id::create(self.as_str());
        let selector: Box<dyn PlanSelector> = match self {
            Self::KeepLastSelected => Box::new(KeepLastSelector),
            Self::BestScore => Box::new(BestScoreSelector),
            Self::SelectRandom => Box::new(RandomSelector),
        };

        Box::new(GenericPlanStrategy {
            name,
            selector,
            modules: Vec::new(),
        })
    }
}

impl fmt::Display for DefaultSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DefaultSelector {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "KeepLastSelected" => Ok(Self::KeepLastSelected),
            "BestScore" => Ok(Self::BestScore),
            "SelectRandom" => Ok(Self::SelectRandom),
            _ => Err(format!("Unknown DefaultSelector: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefaultStrategy {
    ReRoute,
}

impl DefaultStrategy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReRoute => "ReRoute",
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
            "ReRoute" => Ok(Self::ReRoute),
            _ => Err(format!("Unknown DefaultStrategy: {value}")),
        }
    }
}

pub(crate) fn replan_population(
    population: Population,
    iteration: u32,
    base_seed: u64,
) -> Population {
    let manager = StrategyManager::default();
    let persons = population
        .persons
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(id, mut person)| {
            manager.run(iteration, base_seed, &mut person);
            (id, person)
        })
        .collect();

    Population { persons }
}

#[allow(dead_code)]
#[derive(Builder)]
#[builder(pattern = "owned")]
pub(crate) struct StrategyManager {
    #[builder(default = "default_weights_per_subpopulation()")]
    weights_per_subpopulation: HashMap<Id<String>, StrategyWeights>,
    #[builder(default = "default_max_memory_size()")]
    max_memory_size: usize,
    #[builder(default = "default_plan_remover()")]
    plan_remover: Box<dyn PlanSelector>,
    #[builder(default = "default_strategies()")]
    strategies: HashMap<Id<String>, Box<dyn PlanStrategy>>,
}

impl StrategyManager {
    pub(crate) fn builder() -> StrategyManagerBuilder {
        StrategyManagerBuilder::default()
    }

    fn run(&self, iteration: u32, base_seed: u64, person: &mut InternalPerson) {
        let context = ReplanningContext {
            iteration,
            base_seed,
        };
        if let Some(strategy) = self.choose_strategy(&context, person) {
            strategy.handle(person, &context);
        }
        self.remove_plans_if_needed(person, &context);
    }

    fn choose_strategy(
        &self,
        context: &ReplanningContext,
        person: &InternalPerson,
    ) -> Option<&dyn PlanStrategy> {
        let weights = self.weights_per_subpopulation.get(person.subpopulation())?;
        let total_weight = weights.total_weight();
        if total_weight <= 0.0 {
            return None;
        }

        let mut rng = get_rng(
            context.base_seed,
            (
                context.iteration,
                person.id().external(),
                STRATEGY_RNG_PURPOSE,
            ),
        );

        // Weighted random selection over positive strategy weights.
        let mut draw = rng.random_range(0.0..total_weight);
        for entry in weights.entries.iter().filter(|entry| entry.weight > 0.0) {
            if draw < entry.weight {
                return Some(self.strategy_by_name(&entry.strategy_name));
            }
            draw -= entry.weight;
        }

        weights
            .entries
            .iter()
            .rev()
            .find(|entry| entry.weight > 0.0)
            .map(|entry| self.strategy_by_name(&entry.strategy_name))
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
        StrategyManager::builder().build().unwrap()
    }
}

fn default_weights_per_subpopulation() -> HashMap<Id<String>, StrategyWeights> {
    let mut weights_per_subpopulation = HashMap::default();
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
    Box::new(WorstSelector)
}

fn default_strategies() -> HashMap<Id<String>, Box<dyn PlanStrategy>> {
    let mut strategies = HashMap::default();
    for selector in [
        DefaultSelector::KeepLastSelected,
        DefaultSelector::BestScore,
        DefaultSelector::SelectRandom,
    ] {
        strategies.insert(
            Id::create(selector.as_str()),
            selector.generic_plan_strategy(),
        );
    }
    strategies
}

struct StrategyWeights {
    entries: Vec<StrategyWeight>,
}

impl StrategyWeights {
    fn new(entries: Vec<StrategyWeight>) -> Self {
        Self { entries }
    }

    fn total_weight(&self) -> f64 {
        self.entries
            .iter()
            .filter(|entry| entry.weight > 0.0)
            .map(|entry| entry.weight)
            .sum()
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

#[allow(dead_code)]
// This is responsible for picking a plan, copying it and replanning it.
trait PlanStrategy: Send + Sync {
    fn name(&self) -> &Id<String>;
    fn handle(&self, person: &mut InternalPerson, context: &ReplanningContext);
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
// This is the smallest replanning unit (e.g., routes a plan).
trait PlanStrategyModule: Send + Sync {
    fn handle(&self, person: &mut InternalPerson, plan_index: usize);
}

#[allow(dead_code)]
struct ReRouteModule {
    // hold reference to scenario
    // hold reference to router
}

impl PlanStrategyModule for ReRouteModule {
    fn handle(&self, _person: &mut InternalPerson, _plan_index: usize) {
        //extract the trips from the plan
        //extract vehicle from the scenario
        //call the router correspondingly
        todo!()
    }
}

struct ReplanningContext {
    iteration: u32,
    base_seed: u64,
}

trait PlanSelector: Send + Sync {
    fn select(&self, person: &InternalPerson, context: &ReplanningContext) -> usize;
}

struct KeepLastSelector;

impl PlanSelector for KeepLastSelector {
    fn select(&self, person: &InternalPerson, _context: &ReplanningContext) -> usize {
        let mut selected = person
            .plans()
            .iter()
            .enumerate()
            .filter(|(_, plan)| plan.selected);
        let (index, _) = selected
            .next()
            .expect("KeepLastSelector could not find a selected plan.");
        assert!(
            selected.next().is_none(),
            "KeepLastSelector found multiple selected plans."
        );
        index
    }
}

#[allow(dead_code)]
struct BestScoreSelector;

impl PlanSelector for BestScoreSelector {
    fn select(&self, person: &InternalPerson, _context: &ReplanningContext) -> usize {
        person
            .plans()
            .iter()
            .enumerate()
            .filter_map(|(index, plan)| plan.score.map(|score| (index, score)))
            .max_by(|(_, left), (_, right)| left.total_cmp(right))
            .map(|(index, _)| index)
            .expect("BestScoreSelector could not find a scored plan.")
    }
}

#[allow(dead_code)]
struct RandomSelector;

impl PlanSelector for RandomSelector {
    fn select(&self, person: &InternalPerson, context: &ReplanningContext) -> usize {
        let plan_count = person.plans().len();
        assert!(plan_count > 0, "RandomSelector could not find a plan.");
        let mut rng = get_rng(
            context.base_seed,
            (
                context.iteration,
                person.id().external(),
                RANDOM_SELECTOR_RNG_PURPOSE,
            ),
        );
        rng.random_range(0..plan_count)
    }
}

struct WorstSelector;

impl PlanSelector for WorstSelector {
    fn select(&self, person: &InternalPerson, _context: &ReplanningContext) -> usize {
        let plans = person.plans();
        let prefer_unselected = plans.iter().any(|plan| !plan.selected);
        let mut worst_index = None;

        for (index, plan) in plans.iter().enumerate() {
            if prefer_unselected && plan.selected {
                continue;
            }

            let Some(current_worst_index) = worst_index else {
                worst_index = Some(index);
                continue;
            };

            if plan_is_worse(plan.score, plans[current_worst_index].score) {
                worst_index = Some(index);
            }
        }

        worst_index.expect("WorstSelector could not find a removable plan.")
    }
}

fn plan_is_worse(candidate: Option<f64>, current: Option<f64>) -> bool {
    match (score_for_ordering(candidate), score_for_ordering(current)) {
        (None, Some(_)) => true,
        (Some(_), None) | (None, None) => false,
        (Some(candidate), Some(current)) => candidate < current,
    }
}

fn score_for_ordering(score: Option<f64>) -> Option<f64> {
    score.filter(|score| !score.is_nan())
}

#[cfg(test)]
mod tests {
    use super::{
        DefaultSelector, GenericPlanStrategy, KeepLastSelector, PlanSelector, PlanStrategy,
        PlanStrategyModule, RandomSelector, ReplanningContext, StrategyManager, WorstSelector,
    };
    use crate::simulation::id::Id;
    use crate::simulation::scenario::population::{InternalPerson, InternalPlan};

    #[test]
    fn keep_last_selector_returns_selected_plan_index() {
        let person = person_with_scores([Some(1.0), Some(2.0)]);

        assert_eq!(0, KeepLastSelector.select(&person, &context()));
    }

    #[test]
    #[should_panic(expected = "KeepLastSelector could not find a selected plan.")]
    fn keep_last_selector_panics_without_selected_plan() {
        let mut person = person_with_scores([Some(1.0), Some(2.0)]);
        for plan in person.plans_mut() {
            plan.selected = false;
        }

        KeepLastSelector.select(&person, &context());
    }

    #[test]
    #[should_panic(expected = "KeepLastSelector found multiple selected plans.")]
    fn keep_last_selector_panics_with_multiple_selected_plans() {
        let mut person = person_with_scores([Some(1.0), Some(2.0)]);
        person.plans_mut()[1].selected = true;

        KeepLastSelector.select(&person, &context());
    }

    #[test]
    fn worst_selector_treats_missing_score_as_worst() {
        let person = person_with_scores([Some(1.0), None, Some(-5.0)]);

        assert_eq!(1, WorstSelector.select(&person, &context()));
    }

    #[test]
    fn worst_selector_prefers_removing_unselected_plans() {
        let mut person = person_with_scores([Some(-100.0), Some(1.0)]);
        person.plans_mut()[0].selected = true;
        person.plans_mut()[1].selected = false;

        assert_eq!(1, WorstSelector.select(&person, &context()));
    }

    #[test]
    fn default_selectors_create_generic_strategies_with_matching_names() {
        for selector in [
            DefaultSelector::KeepLastSelected,
            DefaultSelector::BestScore,
            DefaultSelector::SelectRandom,
        ] {
            let strategy = selector.generic_plan_strategy();

            assert_eq!(&Id::create(selector.as_str()), strategy.name());
        }
    }

    #[test]
    fn random_selector_is_deterministic_for_same_context() {
        let person = person_with_scores([Some(1.0), Some(2.0), Some(3.0)]);
        let context = context();

        let first = RandomSelector.select(&person, &context);
        let second = RandomSelector.select(&person, &context);

        assert_eq!(first, second);
        assert!(first < person.plans().len());
    }

    #[test]
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

    #[test]
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

    #[test]
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

    fn person_with_scores<const N: usize>(scores: [Option<f64>; N]) -> InternalPerson {
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

    fn context() -> ReplanningContext {
        ReplanningContext {
            iteration: 7,
            base_seed: 42,
        }
    }

    struct MarkCopiedPlanModule;

    impl PlanStrategyModule for MarkCopiedPlanModule {
        fn handle(&self, person: &mut InternalPerson, plan_index: usize) {
            person.plans_mut()[plan_index].score = Some(99.0);
        }
    }
}
