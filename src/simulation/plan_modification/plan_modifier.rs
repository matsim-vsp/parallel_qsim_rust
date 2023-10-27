use mpi::topology::SystemCommunicator;
use tracing::debug;

use crate::simulation::id::{Id, IdStore};
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::{Activity, Agent, Leg};
use crate::simulation::network::global_network::Network;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::plan_modification::routing::router::Router;
use crate::simulation::plan_modification::routing::travel_times_collecting_alt_router::TravelTimesCollectingAltRouter;
use crate::simulation::plan_modification::walk_finder::{EuclideanWalkFinder, WalkFinder};
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
    walk_finder: Box<dyn WalkFinder>,
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
        let walk_finder: Box<dyn WalkFinder> =
            Box::new(EuclideanWalkFinder::new(walking_speed_in_m_per_sec));

        PathFindingPlanModifier {
            router,
            walk_finder,
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

        let mode_id = network.modes.get(agent.next_leg().mode);
        let vehicle_id = garage.get_mode_veh_id(agent_id, &mode_id);
        let distance = Self::calculate_distance(&route, &network);

        agent.update_next_leg(
            dep_time,
            travel_time.unwrap(),
            route,
            distance,
            vehicle_id.internal(),
        );
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

        let dep_time;

        let walk = if agent.curr_act().is_interaction(act_type_id_store) {
            dep_time = curr_act.end_time;
            self.walk_finder.find_walk(next_act, network)
        } else {
            dep_time = curr_act.end_time;
            self.walk_finder.find_walk(curr_act, network)
        };

        let mode_id = network.modes.get(agent.next_leg().mode);
        let vehicle_id = garage.get_mode_veh_id(agent_id, &mode_id);

        agent.update_next_leg(
            dep_time,
            walk.duration,
            vec![],
            walk.distance,
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

    fn calculate_distance(route: &Vec<u64>, network: &Network) -> f64 {
        let distance: f64 = route
            .iter()
            .map(|l| network.link_ids.get(*l))
            .map(|id| {
                network
                    .links
                    .iter()
                    .find(|l| l.id == id)
                    .expect(&*format!("No link with id {:?}", id))
            })
            .map(|l| l.length)
            .sum();
        distance
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::{Id, IdStore};
    use crate::simulation::messaging::messages::proto::{Agent, Route};
    use crate::simulation::network::global_network::Network;
    use crate::simulation::network::sim_network::SimNetworkPartition;
    use crate::simulation::plan_modification::plan_modifier::{
        PathFindingPlanModifier, PlanModifier,
    };
    use crate::simulation::population::population::{ActType, Population};
    use crate::simulation::vehicles::garage::Garage;

    #[test]
    fn test_dummy_leg() {
        //prepare
        let mut network =
            Network::from_file("./assets/adhoc_routing/no_updates/network.xml", 1, "metis");
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml", &mut network.modes);
        let mut population = Population::from_file(
            "./assets/adhoc_routing/no_updates/agents.xml",
            &network,
            &mut garage,
            0,
        );
        let sim_net = SimNetworkPartition::from_network(&network, 0);
        let agent_id = population.agent_ids.get(0);
        let mut agent = population.agents.get_mut(&agent_id).unwrap();

        let plan_modifier = PathFindingPlanModifier::new(&sim_net, &garage);

        //do change
        plan_modifier.update_agent(
            0,
            &mut agent,
            &agent_id,
            &population.act_types,
            &network,
            &garage,
        );

        //check activities
        assert_eq!(agent.plan.as_ref().unwrap().acts.len(), 4);
        assert_eq!(
            get_act_type_id(&population.act_types, &agent, 1).external(),
            "car interaction"
        );
        assert_eq!(
            get_act_type_id(&population.act_types, &agent, 2).external(),
            "car interaction"
        );

        //check legs
        assert_eq!(agent.plan.as_ref().unwrap().legs.len(), 3);
        assert_eq!(get_mode_id(&network, &agent, 0).external(), "walk");
        assert_eq!(get_mode_id(&network, &agent, 1).external(), "car");
        assert_eq!(get_mode_id(&network, &agent, 2).external(), "walk");
    }

    #[test]
    fn test_update_walk_leg() {
        //prepare
        let mut network = Network::from_file("./assets/3-links/3-links-network.xml", 1, "metis");
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml", &mut network.modes);
        let mut population = Population::from_file(
            "./assets/3-links/1-agent-full-leg-dummy.xml",
            &network,
            &mut garage,
            0,
        );
        let sim_net = SimNetworkPartition::from_network(&network, 0);
        let agent_id = population.agent_ids.get(0);
        let mut agent = population.agents.get_mut(&agent_id).unwrap();

        let plan_modifier = PathFindingPlanModifier::new(&sim_net, &garage);

        //do change
        plan_modifier.update_agent(
            0,
            &mut agent,
            &agent_id,
            &population.act_types,
            &network,
            &garage,
        );

        //check activities
        assert_eq!(agent.plan.as_ref().unwrap().acts.len(), 4);
        assert_eq!(
            get_act_type_id(&population.act_types, &agent, 1).external(),
            "car interaction"
        );
        assert_eq!(
            get_act_type_id(&population.act_types, &agent, 2).external(),
            "car interaction"
        );

        //check legs
        assert_eq!(agent.plan.as_ref().unwrap().legs.len(), 3);
        assert_eq!(get_mode_id(&network, &agent, 0).external(), "walk");
        assert_eq!(get_mode_id(&network, &agent, 1).external(), "car");
        assert_eq!(get_mode_id(&network, &agent, 2).external(), "walk");

        let access_leg = agent.plan.as_ref().unwrap().legs.get(0);

        assert_eq!(access_leg.unwrap().trav_time, 8);
        assert_eq!(
            access_leg
                .as_ref()
                .unwrap()
                .route
                .as_ref()
                .unwrap()
                .distance,
            10.
        );
    }

    #[test]
    fn test_update_main_leg() {
        //prepare
        let mut network = Network::from_file("./assets/3-links/3-links-network.xml", 1, "metis");
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml", &mut network.modes);
        let mut population = Population::from_file(
            "./assets/3-links/1-agent-full-leg-dummy.xml",
            &network,
            &mut garage,
            0,
        );
        let sim_net = SimNetworkPartition::from_network(&network, 0);
        let agent_id = population.agent_ids.get(0);
        let mut agent = population.agents.get_mut(&agent_id).unwrap();

        let plan_modifier = PathFindingPlanModifier::new(&sim_net, &garage);

        //do change of walk leg
        plan_modifier.update_agent(
            0,
            &mut agent,
            &agent_id,
            &population.act_types,
            &network,
            &garage,
        );

        //agent is on leg now
        agent.advance_plan();

        //agent is performing car interaction
        agent.advance_plan();

        //do change
        plan_modifier.update_agent(
            0,
            &mut agent,
            &agent_id,
            &population.act_types,
            &network,
            &garage,
        );

        //check main leg
        let main_leg = agent.plan.as_ref().unwrap().legs.get(1);
        assert_eq!(
            main_leg.unwrap().route.as_ref().unwrap(),
            &Route {
                veh_id: 0,
                distance: 1200.,
                route: vec![0, 1, 2],
            }
        );
    }

    fn get_act_type_id(
        act_types: &IdStore<ActType>,
        agent: &Agent,
        act_index: usize,
    ) -> Id<ActType> {
        act_types.get(
            agent
                .plan
                .as_ref()
                .unwrap()
                .acts
                .get(act_index)
                .unwrap()
                .act_type,
        )
    }

    fn get_mode_id(network: &Network, agent: &Agent, leg_index: usize) -> Id<String> {
        network.modes.get(
            agent
                .plan
                .as_ref()
                .unwrap()
                .legs
                .get(leg_index)
                .unwrap()
                .mode,
        )
    }
}
