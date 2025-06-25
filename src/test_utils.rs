use std::fs;
use std::path::PathBuf;

use crate::simulation::id::Id;
use crate::simulation::population::{
    InternalActivity, InternalGenericRoute, InternalLeg, InternalNetworkRoute, InternalPerson,
    InternalPlan, InternalRoute,
};
use crate::simulation::vehicles::InternalVehicleType;
use crate::simulation::{config, InternalSimulationAgent, InternalSimulationAgentLogic};

pub fn create_agent_without_route(id: u64) -> InternalSimulationAgent {
    //inserting a dummy route
    create_agent(id, vec![0, 1])
}

pub fn create_agent(id: u64, route: Vec<u64>) -> InternalSimulationAgent {
    let route = InternalNetworkRoute {
        generic_delegate: InternalGenericRoute {
            start_link: Id::create(&*route.first().unwrap().to_string()),
            end_link: Id::create(&*route.first().unwrap().to_string()),
            trav_time: None,
            distance: None,
            vehicle: None,
        },
        route: route
            .into_iter()
            .map(|u| u.to_string())
            .map(|s| Id::create(s.as_ref()))
            .collect(),
    };

    let leg = InternalLeg::new(InternalRoute::Network(route), "car", 0, None);
    let act = InternalActivity::new(0., 0., "act", Id::create("1"), None, None, None);
    let mut plan = InternalPlan::default();
    plan.add_act(act);
    plan.add_leg(leg);
    let person = InternalPerson::new(Id::create(id.to_string().as_str()), plan);

    let mut agent = InternalSimulationAgent {
        logic: InternalSimulationAgentLogic {
            basic_agent_delegate: person,
        },
    };
    agent.advance_plan();

    agent
}

pub fn create_folders(path: PathBuf) -> PathBuf {
    fs::create_dir_all(&path)
        .unwrap_or_else(|_e| panic!("Failed to create folders for path {path:?}"));
    path
}

pub fn create_vehicle_type(
    id: &Id<InternalVehicleType>,
    net_mode: Id<String>,
) -> InternalVehicleType {
    InternalVehicleType {
        id: id.clone(),
        length: 0.0,
        width: 0.0,
        max_v: 0.0,
        pce: 0.0,
        fef: 0.0,
        net_mode,
        attributes: None,
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
