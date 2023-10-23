use geo::{Closest, ClosestPoint, EuclideanDistance, Line, Point};

use crate::simulation::messaging::messages::proto::{Activity, Agent};
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::population::population::Population;
use crate::simulation::vehicles::garage::Garage;

pub trait WalkLegUpdater {
    fn update_walk_leg(&self, agent: &mut Agent, network: &SimNetworkPartition);
}

pub struct EuclideanWalkLegUpdater {
    walking_speed_in_m_per_sec: f32,
}

impl EuclideanWalkLegUpdater {
    pub fn new(walking_speed_in_m_per_sec: f32) -> Self {
        Self {
            walking_speed_in_m_per_sec,
        }
    }

    fn get_walk_distance(&self, curr_act: &Activity, network: &SimNetworkPartition) -> f32 {
        let curr_act_point = Point::new(curr_act.x, curr_act.y);
        let from_node_id = network
            .links
            .get(&curr_act.link_id) //TODO is it correct?
            .unwrap()
            .from();

        let to_node_id = network
            .links
            .get(&curr_act.link_id) //TODO is it correct?
            .unwrap()
            .to();

        let from_node_x = network.global_network.get_node(from_node_id).x;
        let from_node_y = network.global_network.get_node(from_node_id).y;

        let to_node_x = network.global_network.get_node(to_node_id).x;
        let to_node_y = network.global_network.get_node(to_node_id).y;

        let from_point = Point::new(from_node_x, from_node_y);
        let to_point = Point::new(to_node_x, to_node_y);
        let line = Line::new(from_point, to_point);

        let closest = match line.closest_point(&curr_act_point) {
            Closest::Intersection(p) => p,
            Closest::SinglePoint(p) => p,
            Closest::Indeterminate => {
                panic!("Couldn't find closest point.")
            }
        };
        curr_act_point.euclidean_distance(&closest)
    }
}

impl WalkLegUpdater for EuclideanWalkLegUpdater {
    fn update_walk_leg(&self, agent: &mut Agent, network: &SimNetworkPartition) {
        let curr_act = agent.curr_act();
        let next_act = agent.next_act();

        assert_eq!(curr_act.link_id, next_act.link_id);
        //TODO
        //assert_eq!(agent.next_leg().mode, "walk");

        let dep_time;

        let distance = if agent.curr_act().is_interaction() {
            dep_time = curr_act.end_time;
            self.get_walk_distance(next_act, network)
        } else {
            dep_time = curr_act.end_time;
            self.get_walk_distance(curr_act, network)
        };

        let walking_time_in_sec = distance / self.walking_speed_in_m_per_sec;

        //TODO
        agent.update_next_leg(
            dep_time,
            Some(walking_time_in_sec as u32),
            vec![],
            Some(distance),
            &Population::new(),
            &Garage::new(),
        );
    }
}
