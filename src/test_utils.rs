use std::fs;
use std::path::PathBuf;

use crate::simulation::wire_types::population::{Activity, Leg, Person, Plan, Route};

pub fn create_agent(id: u64, route: Vec<u64>) -> Person {
    let route = Route {
        veh_id: id,
        distance: 0.0,
        route,
    };
    let leg = Leg::new(route, 0, 0, None);
    let act = Activity::new(0., 0., 0, 1, None, None, None);
    let mut plan = Plan::new();
    plan.add_act(act);
    plan.add_leg(leg);
    let mut agent = Person::new(id, plan);
    agent.advance_plan();

    agent
}

pub fn create_folders(path: PathBuf) -> PathBuf {
    fs::create_dir_all(&path)
        .unwrap_or_else(|e| panic!("Failed to create folders for path {path:?}"));
    path
}
