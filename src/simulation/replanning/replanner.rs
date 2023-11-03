use std::rc::Rc;
use tracing::debug;

use crate::simulation::id::Id;
use crate::simulation::messaging::communication::communicators::SimCommunicator;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::{Activity, Agent, Leg};
use crate::simulation::network::global_network::{Link, Network};
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::replanning::routing::router::Router;
use crate::simulation::replanning::routing::travel_times_collecting_alt_router::TravelTimesCollectingAltRouter;
use crate::simulation::replanning::walk_finder::{EuclideanWalkFinder, WalkFinder};
use crate::simulation::vehicles::garage::Garage;

pub trait Replanner {
    fn update_time(&mut self, now: u32, events: &mut EventsPublisher);
    fn replan(&self, now: u32, agent: &mut Agent, garage: &Garage);
}

#[derive(Eq, PartialEq)]
enum LegType {
    AccessEgress,
    Main,
    TripPlaceholder,
}

pub struct DummyReplanner {}

impl Replanner for DummyReplanner {
    fn update_time(&mut self, _now: u32, _events: &mut EventsPublisher) {}

    fn replan(&self, _now: u32, _agent: &mut Agent, _garage: &Garage) {}
}

pub struct ReRouteTripReplanner {
    router: Box<dyn Router>,
    walk_finder: Box<dyn WalkFinder>,
    global_network: Network,
}

impl Replanner for ReRouteTripReplanner {
    fn update_time(&mut self, now: u32, events: &mut EventsPublisher) {
        self.router.next_time_step(now, events)
    }

    fn replan(&self, _now: u32, agent: &mut Agent, garage: &Garage) {
        let leg_type = Self::get_leg_type(agent);
        if leg_type == LegType::TripPlaceholder {
            self.insert_access_egress(agent);
        }

        match leg_type {
            LegType::AccessEgress | LegType::TripPlaceholder => {
                self.replan_access_egress(agent, garage)
            }
            LegType::Main => self.replan_main(agent, garage),
        };
    }
}

impl ReRouteTripReplanner {
    pub fn new<C: SimCommunicator + 'static>(
        global_network: &Network,
        sim_network: &SimNetworkPartition,
        garage: &Garage,
        communicator: Rc<C>,
    ) -> ReRouteTripReplanner {
        let forward_backward_graph_by_mode =
            TravelTimesCollectingAltRouter::<C>::get_forward_backward_graph_by_mode(
                global_network,
                &garage.vehicle_types,
            );

        let router: Box<dyn Router> = Box::new(TravelTimesCollectingAltRouter::new(
            forward_backward_graph_by_mode,
            communicator,
            sim_network.get_link_ids(),
        ));

        let walk_finder: Box<dyn WalkFinder> = Box::new(EuclideanWalkFinder::new());

        ReRouteTripReplanner {
            router,
            walk_finder,
            global_network: global_network.clone(),
        }
    }

    fn insert_access_egress(&self, agent: &mut Agent) {
        // So far, we have:
        // act (current) - leg (next) - act (next)
        //
        // We want:
        // act (current) - walk (next)  - interaction act (next) - leg - interaction act - walk - act
        //
        // Thus, we need to
        // 1. insert 2 interaction activities between current and next activity
        // 2. insert access and egress walking legs before and after main leg

        // mode on next_leg() is an internal id of network.modes, NOT of garage.vehicle_types
        // we assume that there is exactly one network mode for each vehicle type
        let main_leg_mode = String::from(Id::<String>::get(agent.next_leg().mode).external());
        let id = Id::<String>::get_from_ext(&format!("{} interaction", main_leg_mode));

        let new_acts = vec![
            Activity::interaction(agent.curr_act().link_id, id.internal()),
            Activity::interaction(agent.next_act().link_id, id.internal()),
        ];
        agent.add_act_after_curr(new_acts);

        //"walk" as default access egress mode is hard coded here. Could also be optional
        let access_egress_mode_id = Id::<String>::get_from_ext("walk").internal();

        //replace current leg()
        let access = Leg::access_eggress(access_egress_mode_id);
        let egress = Leg::access_eggress(access_egress_mode_id);

        //we have: last leg (current) - main leg (next)
        //we want: last leg (current) - walk access leg (next) - main leg - walk egress leg
        agent.replace_next_leg(vec![access, agent.next_leg().clone(), egress]);
    }

    fn replan_main(&self, agent: &mut Agent, garage: &Garage) {
        let curr_act = agent.curr_act();

        let (route, travel_time) =
            self.find_route(agent.curr_act(), agent.next_act(), agent.next_leg().mode);
        let dep_time = curr_act.end_time;

        let mode_id = Id::get(agent.next_leg().mode);
        let vehicle_id = garage.get_mode_veh_id(&Id::get(agent.id), &mode_id);
        let distance = self.calculate_distance(&route);

        agent.update_next_leg(
            dep_time,
            travel_time.unwrap(),
            route,
            distance,
            vehicle_id.internal(),
        );
    }

    fn replan_access_egress(&self, agent: &mut Agent, garage: &Garage) {
        let curr_act = agent.curr_act();
        let next_act = agent.next_act();

        assert_eq!(curr_act.link_id, next_act.link_id);

        let main_leg_mode = String::from(Id::<String>::get(agent.next_leg().mode).external());
        let access_egress_speed = garage
            .vehicle_types
            .get(&Id::get_from_ext(&main_leg_mode))
            .unwrap()
            .max_v;

        let dep_time;

        let walk = if curr_act.is_interaction() {
            dep_time = curr_act.end_time;
            self.walk_finder
                .find_walk(next_act, access_egress_speed, &self.global_network)
        } else {
            dep_time = curr_act.end_time;
            self.walk_finder
                .find_walk(curr_act, access_egress_speed, &self.global_network)
        };

        let mode_id = Id::<String>::get(agent.next_leg().mode);
        let vehicle_id = garage.get_mode_veh_id(&Id::<Agent>::get(agent.id), &mode_id);

        agent.update_next_leg(
            dep_time,
            walk.duration,
            vec![agent.curr_act().link_id, agent.curr_act().link_id],
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

    #[allow(clippy::if_same_then_else)]
    fn get_leg_type(agent: &Agent) -> LegType {
        //act - leg - interaction act => walk
        if !agent.curr_act().is_interaction() && agent.next_act().is_interaction() {
            LegType::AccessEgress
        }
        //interaction act - leg - act => walk
        else if agent.curr_act().is_interaction() && !agent.next_act().is_interaction() {
            LegType::AccessEgress
        }
        //interaction act - leg - interaction act => main leg
        else if agent.curr_act().is_interaction() && agent.next_act().is_interaction() {
            LegType::Main
        }
        //act - leg - act => dummy leg
        else if !agent.curr_act().is_interaction() && !agent.next_act().is_interaction() {
            LegType::TripPlaceholder
        } else {
            panic!("Computing a leg between two main activities should never happen.")
        }
    }

    fn calculate_distance(&self, route: &[u64]) -> f64 {
        let distance: f64 = route
            .iter()
            .map(|id| {
                self.global_network
                    .links
                    .iter()
                    .find(|l| l.id == Id::<Link>::get(*id))
                    .unwrap_or_else(|| panic!("No link with id {:?}", id))
            })
            .map(|l| l.length)
            .sum();
        distance
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::PartitionMethod;
    use crate::simulation::id::Id;
    use crate::simulation::messaging::communication::communicators::DummySimCommunicator;
    use crate::simulation::messaging::messages::proto::{Agent, Route};
    use crate::simulation::network::global_network::Network;
    use crate::simulation::network::sim_network::SimNetworkPartition;
    use crate::simulation::population::population::Population;
    use crate::simulation::replanning::replanner::{ReRouteTripReplanner, Replanner};
    use crate::simulation::vehicles::garage::Garage;
    use std::rc::Rc;

    #[test]
    fn test_trip_placeholder_leg() {
        //prepare
        let network = Network::from_file(
            "./assets/adhoc_routing/no_updates/network.xml",
            1,
            PartitionMethod::Metis,
        );
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml");
        let mut population = Population::from_file(
            "./assets/adhoc_routing/no_updates/agents.xml",
            &network,
            &mut garage,
            0,
        );
        let sim_net = SimNetworkPartition::from_network(&network, 0, 1.0);
        let agent_id = Id::get_from_ext("100");
        let mut agent = population.agents.get_mut(&agent_id).unwrap();

        let replanner =
            ReRouteTripReplanner::new(&network, &sim_net, &garage, Rc::new(DummySimCommunicator()));

        //do change
        replanner.replan(0, &mut agent, &garage);

        //check activities
        assert_eq!(agent.plan.as_ref().unwrap().acts.len(), 4);
        assert_eq!(get_act_type_id(&agent, 1).external(), "car interaction");
        assert_eq!(get_act_type_id(&agent, 2).external(), "car interaction");

        //check legs
        assert_eq!(agent.plan.as_ref().unwrap().legs.len(), 3);
        assert_eq!(get_mode_id(&agent, 0).external(), "walk");
        assert_eq!(get_mode_id(&agent, 1).external(), "car");
        assert_eq!(get_mode_id(&agent, 2).external(), "walk");
    }

    #[test]
    fn test_update_walk_leg() {
        //prepare
        let network = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            1,
            PartitionMethod::Metis,
        );
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml");
        let mut population = Population::from_file(
            "./assets/3-links/1-agent-trip-leg.xml",
            &network,
            &mut garage,
            0,
        );
        let sim_net = SimNetworkPartition::from_network(&network, 0, 1.0);
        let agent_id = Id::get_from_ext("100");
        let mut agent = population.agents.get_mut(&agent_id).unwrap();

        let replanner =
            ReRouteTripReplanner::new(&network, &sim_net, &garage, Rc::new(DummySimCommunicator()));

        //do change
        replanner.replan(0, &mut agent, &garage);

        //check activities
        assert_eq!(agent.plan.as_ref().unwrap().acts.len(), 4);
        assert_eq!(get_act_type_id(&agent, 1).external(), "car interaction");
        assert_eq!(get_act_type_id(&agent, 2).external(), "car interaction");

        //check legs
        assert_eq!(agent.plan.as_ref().unwrap().legs.len(), 3);
        assert_eq!(get_mode_id(&agent, 0).external(), "walk");
        assert_eq!(get_mode_id(&agent, 1).external(), "car");
        assert_eq!(get_mode_id(&agent, 2).external(), "walk");

        let access_leg = agent.plan.as_ref().unwrap().legs.get(0);

        assert_eq!(
            access_leg.unwrap().trav_time,
            (access_leg.unwrap().route.as_ref().unwrap().distance / 0.85) as u32
        );
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
        let network = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            1,
            PartitionMethod::Metis,
        );
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml");
        let mut population = Population::from_file(
            "./assets/3-links/1-agent-trip-leg.xml",
            &network,
            &mut garage,
            0,
        );
        let sim_net = SimNetworkPartition::from_network(&network, 0, 1.0);
        let agent_id = Id::get_from_ext("100");
        let mut agent = population.agents.get_mut(&agent_id).unwrap();

        let replanner =
            ReRouteTripReplanner::new(&network, &sim_net, &garage, Rc::new(DummySimCommunicator()));

        //do change of walk leg
        replanner.replan(0, &mut agent, &garage);

        //agent is on leg now
        agent.advance_plan();

        //agent is performing car interaction
        agent.advance_plan();

        //do change
        replanner.replan(0, &mut agent, &garage);

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

    fn get_act_type_id(agent: &Agent, act_index: usize) -> Id<String> {
        Id::<String>::get(
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

    fn get_mode_id(agent: &Agent, leg_index: usize) -> Id<String> {
        Id::<String>::get(
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
