use std::fs;
use std::path::PathBuf;

use crate::simulation::config;
use crate::simulation::id::Id;
use crate::simulation::wire_types::messages::{PlanLogic, SimulationAgent, SimulationAgentLogic};
use crate::simulation::wire_types::population::leg::Route;
use crate::simulation::wire_types::population::{
    Activity, GenericRoute, Leg, NetworkRoute, Person, Plan,
};
use crate::simulation::wire_types::vehicles::VehicleType;

pub fn create_agent_without_route(id: u64) -> SimulationAgent {
    //inserting a dummy route
    create_agent(id, vec![0, 1])
}

pub fn create_agent(id: u64, route: Vec<u64>) -> SimulationAgent {
    let route = NetworkRoute {
        delegate: Some(GenericRoute {
            start_link: *route.first().unwrap(),
            end_link: *route.last().unwrap(),
            trav_time: None,
            distance: None,
            veh_id: Some(id),
        }),
        route,
    };
    let leg = Leg::new(Route::NetworkRoute(route), 0, 0, None);
    let act = Activity::new(0., 0., 0, 1, None, None, None);
    let mut plan = Plan::new();
    plan.add_act(act);
    plan.add_leg(leg);
    let person = Person::new(id, plan);

    let mut agent = SimulationAgent {
        agent_logic: Some(SimulationAgentLogic {
            r#type: Some(
                crate::simulation::wire_types::messages::simulation_agent_logic::Type::PlanLogic(
                    PlanLogic {
                        person: Some(person),
                    },
                ),
            ),
        }),
    };
    agent.advance_plan();

    agent
}

pub fn create_folders(path: PathBuf) -> PathBuf {
    fs::create_dir_all(&path)
        .unwrap_or_else(|_e| panic!("Failed to create folders for path {path:?}"));
    path
}

pub fn create_vehicle_type(id: &Id<VehicleType>, net_mode: Id<String>) -> VehicleType {
    VehicleType {
        id: id.internal(),
        length: 0.0,
        width: 0.0,
        max_v: 0.0,
        pce: 0.0,
        fef: 0.0,
        net_mode: net_mode.internal(),
    }
}

pub fn config() -> config::Simulation {
    config::Simulation {
        start_time: 0,
        end_time: 0,
        sample_size: 1.0,
        stuck_threshold: u32::MAX,
        main_modes: vec![String::from("car")],
    }
}
