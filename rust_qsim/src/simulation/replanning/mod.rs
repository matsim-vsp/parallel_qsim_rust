use crate::simulation::id::Id;
use crate::simulation::random::get_rng;
use crate::simulation::scenario::population::{DEFAULT_SUBPOPULATION, InternalPerson, Population};
use ahash::HashMap;
use rand::Rng;
use rayon::prelude::*;

pub mod routing;

const STRATEGY_RNG_PURPOSE: &str = "replanning.strategy";
const KEEP_LAST_SELECTED_STRATEGY: &str = "KeepLastSelected";

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
pub(crate) struct StrategyManager {
    weights_per_subpopulation: HashMap<Id<String>, StrategyWeights>,
    max_memory_size: usize,
    plan_remover: Box<dyn PlanSelector>,
    strategies: HashMap<Id<String>, Box<dyn PlanStrategy>>,
}

impl StrategyManager {
    fn run(&self, iteration: u32, base_seed: u64, person: &mut InternalPerson) {
        if let Some(strategy) = self.choose_strategy(iteration, base_seed, person) {
            strategy.handle(person);
        }
        self.remove_plans_if_needed(person);
    }

    fn choose_strategy(
        &self,
        iteration: u32,
        base_seed: u64,
        person: &InternalPerson,
    ) -> Option<&dyn PlanStrategy> {
        let weights = self.weights_per_subpopulation.get(person.subpopulation())?;
        let total_weight = weights.total_weight();
        if total_weight <= 0.0 {
            return None;
        }

        if let Some(entry) = weights.single_positive_entry() {
            return Some(self.strategy_by_name(&entry.strategy_name));
        }

        // TODO isn't there a bias towards the first entry?
        let mut rng = get_rng(
            base_seed,
            (iteration, person.id().external(), STRATEGY_RNG_PURPOSE),
        );
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

    fn remove_plans_if_needed(&self, person: &mut InternalPerson) {
        while person.plans().len() > self.max_memory_size {
            let index = self.plan_remover.select(person);
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
        let strategy_name = Id::create(KEEP_LAST_SELECTED_STRATEGY);
        let strategy: Box<dyn PlanStrategy> = Box::new(GenericPlanStrategy {
            name: strategy_name.clone(),
            selector: Box::new(KeepLastSelector),
            modules: Vec::new(),
        });

        let mut weights_per_subpopulation = HashMap::default();
        weights_per_subpopulation.insert(
            Id::create(DEFAULT_SUBPOPULATION),
            StrategyWeights {
                entries: vec![StrategyWeight {
                    strategy_name: strategy_name.clone(),
                    weight: 1.0,
                }],
            },
        );
        let mut strategies = HashMap::default();
        strategies.insert(strategy_name, strategy);

        Self {
            weights_per_subpopulation,
            max_memory_size: usize::MAX,
            plan_remover: Box::new(WorstSelector),
            strategies,
        }
    }
}

struct StrategyWeights {
    entries: Vec<StrategyWeight>,
}

impl StrategyWeights {
    fn total_weight(&self) -> f64 {
        self.entries
            .iter()
            .filter(|entry| entry.weight > 0.0)
            .map(|entry| entry.weight)
            .sum()
    }

    fn single_positive_entry(&self) -> Option<&StrategyWeight> {
        let mut positive_entries = self.entries.iter().filter(|entry| entry.weight > 0.0);
        let entry = positive_entries.next()?;
        positive_entries.next().is_none().then_some(entry)
    }
}

struct StrategyWeight {
    strategy_name: Id<String>,
    weight: f64,
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
    fn handle(&self, person: &mut InternalPerson);
}

impl PlanStrategy for GenericPlanStrategy {
    fn name(&self) -> &Id<String> {
        &self.name
    }

    fn handle(&self, person: &mut InternalPerson) {
        let plan_index = self.selector.select(person);
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

trait PlanSelector: Send + Sync {
    fn select(&self, person: &InternalPerson) -> usize;
}

struct KeepLastSelector;

impl PlanSelector for KeepLastSelector {
    fn select(&self, person: &InternalPerson) -> usize {
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
    fn select(&self, person: &InternalPerson) -> usize {
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
    fn select(&self, person: &InternalPerson) -> usize {
        person
            .plans()
            .iter()
            .position(|plan| plan.selected)
            .expect("RandomSelector could not find a selected plan.")
    }
}

struct WorstSelector;

impl PlanSelector for WorstSelector {
    fn select(&self, person: &InternalPerson) -> usize {
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
        GenericPlanStrategy, KEEP_LAST_SELECTED_STRATEGY, KeepLastSelector, PlanSelector,
        PlanStrategy, PlanStrategyModule, StrategyManager, WorstSelector,
    };
    use crate::simulation::id::Id;
    use crate::simulation::scenario::population::{InternalPerson, InternalPlan};

    #[test]
    fn keep_last_selector_returns_selected_plan_index() {
        let person = person_with_scores([Some(1.0), Some(2.0)]);

        assert_eq!(0, KeepLastSelector.select(&person));
    }

    #[test]
    #[should_panic(expected = "KeepLastSelector could not find a selected plan.")]
    fn keep_last_selector_panics_without_selected_plan() {
        let mut person = person_with_scores([Some(1.0), Some(2.0)]);
        for plan in person.plans_mut() {
            plan.selected = false;
        }

        KeepLastSelector.select(&person);
    }

    #[test]
    #[should_panic(expected = "KeepLastSelector found multiple selected plans.")]
    fn keep_last_selector_panics_with_multiple_selected_plans() {
        let mut person = person_with_scores([Some(1.0), Some(2.0)]);
        person.plans_mut()[1].selected = true;

        KeepLastSelector.select(&person);
    }

    #[test]
    fn worst_selector_treats_missing_score_as_worst() {
        let person = person_with_scores([Some(1.0), None, Some(-5.0)]);

        assert_eq!(1, WorstSelector.select(&person));
    }

    #[test]
    fn worst_selector_prefers_removing_unselected_plans() {
        let mut person = person_with_scores([Some(-100.0), Some(1.0)]);
        person.plans_mut()[0].selected = true;
        person.plans_mut()[1].selected = false;

        assert_eq!(1, WorstSelector.select(&person));
    }

    #[test]
    fn default_strategy_is_named_generic_keep_last_selected() {
        let manager = StrategyManager::default();
        let strategy_name = Id::create(KEEP_LAST_SELECTED_STRATEGY);
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

        strategy.handle(&mut person);

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

        strategy.handle(&mut person);

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

    struct MarkCopiedPlanModule;

    impl PlanStrategyModule for MarkCopiedPlanModule {
        fn handle(&self, person: &mut InternalPerson, plan_index: usize) {
            person.plans_mut()[plan_index].score = Some(99.0);
        }
    }
}
