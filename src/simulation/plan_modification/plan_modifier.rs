use mpi::topology::SystemCommunicator;
use tracing::debug;

use crate::simulation::id::{Id, IdStore};
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::{Activity, Agent, Leg};
use crate::simulation::network::global_network::Network;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::plan_modification::routing::router::Router;
use crate::simulation::plan_modification::routing::travel_times_collecting_alt_router::TravelTimesCollectingAltRouter;
use crate::simulation::plan_modification::walk_calculator::{
    EuclideanWalkCalculator, WalkCalculator,
};
use crate::simulation::population::population::ActType;
use crate::simulation::vehicles::garage::Garage;

pub trait PlanModifier {
    fn next_time_step(&self, now: u32, events: &mut EventsPublisher);
    fn update_agent(
        &self,
        now: u32,
        agent: &mut Agent,
        agent_id: &Id<Agent>,
        activity_type_id_store: &IdStore<ActType>,
        network: &Network,
        garage: &Garage,
    );
}

enum LegModificationType {
    WalkLeg,
    MainLeg,
    DummyMainLeg,
}

pub struct PathFindingPlanModifier {
    router: Box<dyn Router>,
    walk_leg_updater: Box<dyn WalkCalculator>,
}

impl PlanModifier for PathFindingPlanModifier {
    fn next_time_step(&self, _now: u32, _events: &mut EventsPublisher) {
        todo!()
    }

    fn update_agent(
        &self,
        _now: u32,
        agent: &mut Agent,
        agent_id: &Id<Agent>,
        act_type_id_store: &IdStore<ActType>,
        network: &Network,
        garage: &Garage,
    ) {
        match Self::get_leg_type(agent, act_type_id_store) {
            LegModificationType::WalkLeg => {
                self.update_walk_leg(agent, agent_id, act_type_id_store, network, garage)
            }
            LegModificationType::MainLeg => self.update_main_leg(agent, agent_id, network, garage),
            LegModificationType::DummyMainLeg => {
                self.update_dummy_leg(agent, act_type_id_store, network)
            }
        }
    }
}

impl PathFindingPlanModifier {
    pub fn new(network: &SimNetworkPartition, garage: &Garage) -> PathFindingPlanModifier {
        let forward_backward_graph_by_mode =
            TravelTimesCollectingAltRouter::get_forward_backward_graph_by_mode(
                &network.global_network,
                &garage.vehicle_types,
            );

        //TODO
        let router: Box<dyn Router> = Box::new(TravelTimesCollectingAltRouter::new(
            forward_backward_graph_by_mode,
            SystemCommunicator::world(),
            42,
            network.get_link_ids(),
        ));

        let walking_speed_in_m_per_sec = 1.2;
        let walk_leg_updater: Box<dyn WalkCalculator> =
            Box::new(EuclideanWalkCalculator::new(walking_speed_in_m_per_sec));

        PathFindingPlanModifier {
            router,
            walk_leg_updater,
        }
    }

    fn update_dummy_leg(
        &self,
        agent: &mut Agent,
        act_type_id_store: &IdStore<ActType>,
        network: &Network,
    ) {
        // So far, we have:
        // act (current) - leg (next) - act (next)
        //
        // We want:
        // act (current) - walk (next)  - interaction act (next) - leg - interaction act - walk - act
        //
        // Thus, we need to
        // 1. insert 2 interaction activities between current and next activity
        // 2. insert access and egress walking legs before and after main leg
        //
        // Anything else (routing and walk finding) is performed at next time step

        // Maybe we should move the creation of interaction and walk ids to some router preparing step.
        // I'm not sure.
        let main_leg_mode = String::from(network.modes.get(agent.next_leg().mode).external());
        let id = act_type_id_store.get_from_ext(&format!("{} interaction", main_leg_mode));

        let new_acts = vec![
            Activity::dummy(agent.curr_act().link_id, id.internal()),
            Activity::dummy(agent.next_act().link_id, id.internal()),
        ];
        agent.add_act_after_curr(new_acts);

        let walk_mode_id = network.modes.get_from_ext("walk").internal();

        let access = Leg::walk_dummy(walk_mode_id);
        let egress = Leg::walk_dummy(walk_mode_id);
        agent.add_access_egress_legs_for_next(access, egress);
    }

    fn update_main_leg(
        &self,
        agent: &mut Agent,
        agent_id: &Id<Agent>,
        network: &Network,
        garage: &Garage,
    ) {
        let curr_act = agent.curr_act();
        let mode = agent.next_leg().routing_mode;

        let (route, travel_time) = self.find_route(agent.curr_act(), agent.next_act(), mode);
        let dep_time = curr_act.end_time;

        let mode_id = network.modes.get(agent.curr_leg().mode);
        let vehicle_id = garage.get_mode_veh_id(agent_id, &mode_id);

        agent.update_next_leg(dep_time, travel_time, route, None, vehicle_id.internal());
    }

    fn update_walk_leg(
        &self,
        agent: &mut Agent,
        agent_id: &Id<Agent>,
        act_type_id_store: &IdStore<ActType>,
        network: &Network,
        garage: &Garage,
    ) {
        let curr_act = agent.curr_act();
        let next_act = agent.next_act();

        assert_eq!(curr_act.link_id, next_act.link_id);
        //TODO
        //assert_eq!(agent.next_leg().mode, "walk");

        let dep_time;

        let walk = if agent.curr_act().is_interaction(act_type_id_store) {
            dep_time = curr_act.end_time;
            self.walk_leg_updater.find_walk(next_act, network)
        } else {
            dep_time = curr_act.end_time;
            self.walk_leg_updater.find_walk(curr_act, network)
        };

        let mode_id = network.modes.get(agent.curr_leg().mode);
        let vehicle_id = garage.get_mode_veh_id(agent_id, &mode_id);

        agent.update_next_leg(
            dep_time,
            Some(walk.duration),
            vec![],
            Some(walk.distance),
            vehicle_id.internal(),
        );
    }

    fn find_route(
        &self,
        from_act: &Activity,
        to_act: &Activity,
        mode: u64,
    ) -> (Vec<u64>, Option<u32>) {
        let query_result = self
            .router
            .query_links(from_act.link_id, to_act.link_id, mode);

        let route = query_result.path.expect("There is no route!");
        let travel_time = query_result.travel_time;

        if route.is_empty() {
            debug!("Route between {:?} and {:?} is empty.", from_act, to_act);
        }

        (route, travel_time)
    }

    fn get_leg_type(agent: &Agent, act_type_id_store: &IdStore<ActType>) -> LegModificationType {
        //act - leg - interaction act => walk
        if !agent.curr_act().is_interaction(act_type_id_store)
            && agent.next_act().is_interaction(act_type_id_store)
        {
            LegModificationType::WalkLeg
        }
        //interaction act - leg - act => walk
        else if agent.curr_act().is_interaction(act_type_id_store)
            && !agent.next_act().is_interaction(act_type_id_store)
        {
            LegModificationType::WalkLeg
        }
        //interaction act - leg - interaction act => main leg
        else if agent.curr_act().is_interaction(act_type_id_store)
            && agent.next_act().is_interaction(act_type_id_store)
        {
            LegModificationType::MainLeg
        }
        //act - leg - act => dummy leg
        else if !agent.curr_act().is_interaction(act_type_id_store)
            && !agent.next_act().is_interaction(act_type_id_store)
        {
            LegModificationType::DummyMainLeg
        } else {
            panic!("Computing a leg between two main activities should never happen.")
        }
    }
}
