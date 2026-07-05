mod test_simulation;

use crate::test_simulation::TestExecutorBuilder;

use macros::integration_test;
use rust_qsim::simulation::config::ScoringPlansCollectionType::{HomeSending, Mapping};
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::io;
use rust_qsim::simulation::scenario::population::{InternalPlan, InternalPlanElement, Population};
use rust_qsim::simulation::scenario::vehicles::Garage;
use std::sync::Arc;

#[integration_test(rust_qsim)]
fn test_scoring_backpacking() {
    let single_config =
        Config::from_args(CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1-scoring.yml"));
    let two_config =
        Config::from_args(CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2-scoring.yml"));
    run_and_verify(single_config, two_config);
}

#[integration_test(rust_qsim)]
fn test_scoring_homesending() {
    let mut single_config =
        Config::from_args(CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1-scoring.yml"));
    single_config.scoring_mut().plans_collection_type = HomeSending;

    let mut two_config =
        Config::from_args(CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2-scoring.yml"));
    two_config.scoring_mut().plans_collection_type = HomeSending;

    run_and_verify(single_config, two_config);
}

#[integration_test(rust_qsim)]
fn test_scoring_mapping() {
    let mut single_config =
        Config::from_args(CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-1-scoring.yml"));
    single_config.scoring_mut().plans_collection_type = Mapping;

    let mut two_config =
        Config::from_args(CommandLineArgs::new_with_path("./tests/resources/equil/equil-config-2-scoring.yml"));
    two_config.scoring_mut().plans_collection_type = Mapping;
    two_config.scoring_mut().collector_threads = 2;

    run_and_verify(single_config, two_config);
}

fn run_and_verify(single_config: Config, two_config: Config) {
    // Single thread
    let single_config = Arc::new(single_config);
    let single_output_dir =
        io::resolve_path(single_config.context(), &single_config.output().output_dir);

    TestExecutorBuilder::default()
        .config(single_config)
        .expected_events(None)
        .build()
        .unwrap()
        .execute();

    // Two threads
    let two_config = Arc::new(two_config);
    let two_output_dir = io::resolve_path(two_config.context(), &two_config.output().output_dir);

    TestExecutorBuilder::default()
        .config(two_config)
        .expected_events(None)
        .build()
        .unwrap()
        .execute();

    let single_thread_population = Population::from_file(
        &single_output_dir.join("plans/output_plans_0.binpb"),
        &mut Garage::new(),
    );
    let two_thread_population_0 = Population::from_file(
        &two_output_dir.join("plans/output_plans_0.binpb"),
        &mut Garage::new(),
    );
    let two_thread_population_1 = Population::from_file(
        &two_output_dir.join("plans/output_plans_1.binpb"),
        &mut Garage::new(),
    );

    for (person_id, person_a) in single_thread_population.persons {
        // Search the person in the two-thread population
        let person_b = if two_thread_population_0.persons.contains_key(&person_id) {
            two_thread_population_0.persons.get(&person_id).unwrap()
        } else {
            two_thread_population_1.persons.get(&person_id).unwrap()
        };

        // Check if both plans pass integrity check
        check_plan_integrity(person_a.plans());
        check_plan_integrity(person_b.plans());

        // Make sure that the plans are equal -> Number of thread should not change the plans
        equal_plans(person_a.plans(), person_b.plans());

        // Make sure, that the plans lie in their home partition
        todo!()
    }
}

/// Checks if the plan components are in correct order and that start/end times do not overlap.
fn check_plan_integrity(plans: &Vec<InternalPlan>) {
    for (plan_idx, plan) in plans.iter().enumerate() {
        let elements = &plan.elements;

        assert!(!elements.is_empty(), "Plan {plan_idx} is empty.");

        // Assertion 1: must start with an activity that has no start_time
        match &elements[0] {
            InternalPlanElement::Activity(act) => {
                assert!(
                    act.start_time.is_none(),
                    "Plan {plan_idx}: first activity must not have a start_time, got {:?}.",
                    act.start_time
                );
            }
            InternalPlanElement::Leg(_) => {
                panic!("Plan {plan_idx}: first element must be an activity, not a leg.")
            }
        }

        // Assertion 2: elements must alternate act/leg/act/leg/...
        for (i, window) in elements.windows(2).enumerate() {
            match (&window[0], &window[1]) {
                (InternalPlanElement::Activity(_), InternalPlanElement::Activity(_)) => {
                    panic!(
                        "Plan {plan_idx}: two consecutive activities at positions {i} and {}.",
                        i + 1
                    );
                }
                (InternalPlanElement::Leg(_), InternalPlanElement::Leg(_)) => {
                    panic!(
                        "Plan {plan_idx}: two consecutive legs at positions {i} and {}.",
                        i + 1
                    );
                }
                _ => {}
            }
        }

        // Assertion 3: times must be contiguous — no gaps or overlaps
        // act.end_time == next_leg.dep_time
        // leg.dep_time + leg.trav_time + 1 == next_act.start_time
        for i in 0..elements.len().saturating_sub(1) {
            match (&elements[i], &elements[i + 1]) {
                (InternalPlanElement::Activity(act), InternalPlanElement::Leg(leg)) => {
                    if let (Some(act_end), Some(dep)) = (act.end_time, leg.dep_time) {
                        assert_eq!(
                            act_end, dep,
                            "Plan {plan_idx}: activity end_time ({act_end}) != leg dep_time ({dep}) at position {i}."
                        );
                    }
                }
                (InternalPlanElement::Leg(leg), InternalPlanElement::Activity(act)) => {
                    if let (Some(dep), Some(trav), Some(act_start)) =
                        (leg.dep_time, leg.trav_time, act.start_time)
                    {
                        assert_eq!(
                            dep + trav + 1,
                            act_start,
                            "Plan {plan_idx}: leg arrival ({dep} + {trav} + 1 = {}) != activity start_time ({act_start}) at position {i}.",
                            dep + trav + 1
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// Compares two plans, returns true if identical
fn equal_plans(plan_a: &Vec<InternalPlan>, plan_b: &Vec<InternalPlan>) {
    todo!()
}
