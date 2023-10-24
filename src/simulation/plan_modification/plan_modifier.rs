use mpi::topology::SystemCommunicator;
use tracing::debug;

use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::{Activity, Agent};
use crate::simulation::network::global_network::Network;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::plan_modification::routing::router::Router;
use crate::simulation::plan_modification::routing::travel_times_collecting_alt_router::TravelTimesCollectingAltRouter;
use crate::simulation::plan_modification::walk_calculator::{
    EuclideanWalkCalculator, WalkCalculator,
};
use crate::simulation::population::population::Population;
use crate::simulation::vehicles::garage::Garage;

pub trait PlanModifier {
    fn next_time_step(&self, now: u32, events: &mut EventsPublisher);
    fn update_agent(&self, now: u32, agent: &mut Agent, x: &Network);
}

pub struct PathFindingPlanModifier {
    router: Box<dyn Router>,
    walk_leg_updater: Box<dyn WalkCalculator>,
}

impl PlanModifier for PathFindingPlanModifier {
    fn next_time_step(&self, _now: u32, _events: &mut EventsPublisher) {
        todo!()
    }

    fn update_agent(&self, _now: u32, agent: &mut Agent, network: &Network) {
        if (!agent.curr_act().is_interaction() && agent.next_act().is_interaction())
            || (agent.curr_act().is_interaction() && !agent.next_act().is_interaction())
        {
            self.update_walk_leg(agent, network);
        } else if agent.curr_act().is_interaction() && agent.next_act().is_interaction() {
            self.update_main_leg(agent);
        } else {
            //TODO
            panic!("Computing a leg between two main activities should never happen.")
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

    fn update_main_leg(&self, agent: &mut Agent) {
        let curr_act = agent.curr_act();
        let mode = agent.next_leg().routing_mode;

        let (route, travel_time) = self.find_route(agent.curr_act(), agent.next_act(), mode);
        let dep_time = curr_act.end_time;

        //TODO
        agent.update_next_leg(
            dep_time,
            travel_time,
            route,
            None,
            &Population::new(),
            &Garage::new(),
        );
    }

    fn update_walk_leg(&self, agent: &mut Agent, network: &Network) {
        let curr_act = agent.curr_act();
        let next_act = agent.next_act();

        assert_eq!(curr_act.link_id, next_act.link_id);
        //TODO
        //assert_eq!(agent.next_leg().mode, "walk");

        let dep_time;

        let walk = if agent.curr_act().is_interaction() {
            dep_time = curr_act.end_time;
            self.walk_leg_updater.find_walk(next_act, network)
        } else {
            dep_time = curr_act.end_time;
            self.walk_leg_updater.find_walk(curr_act, network)
        };

        //TODO
        agent.update_next_leg(
            dep_time,
            Some(walk.duration),
            vec![],
            Some(walk.distance),
            &Population::new(),
            &Garage::new(),
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
}
