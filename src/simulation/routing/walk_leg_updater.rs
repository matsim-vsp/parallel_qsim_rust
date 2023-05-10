use crate::simulation::messaging::messages::proto::{Activity, Agent};
use crate::simulation::network::network_partition::NetworkPartition;
use geo::{Closest, ClosestPoint, EuclideanDistance, Line, Point};

pub trait WalkLegUpdater {
    fn update_walk_leg(&self, agent: &mut Agent, network: &NetworkPartition);
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

    fn get_walk_distance(&self, curr_act: &Activity, network: &NetworkPartition) -> f32 {
        let curr_act_point = Point::new(curr_act.x, curr_act.y);
        let (from_node_id, to_node_id) = network
            .links
            .get(&(curr_act.link_id as usize))
            .unwrap()
            .from_to_id();

        let from_node_x = network.nodes.get(&from_node_id).unwrap().x();
        let from_node_y = network.nodes.get(&from_node_id).unwrap().y();

        let to_node_x = network.nodes.get(&to_node_id).unwrap().x();
        let to_node_y = network.nodes.get(&to_node_id).unwrap().y();

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
    fn update_walk_leg(&self, agent: &mut Agent, network: &NetworkPartition) {
        let curr_act = agent.curr_act();
        let next_act = agent.next_act();

        assert_eq!(curr_act.link_id, next_act.link_id);
        assert_eq!(agent.next_leg().mode, "walk");

        let dep_time;

        let distance = if agent.curr_act().is_interaction() {
            dep_time = curr_act.end_time;
            self.get_walk_distance(next_act, network)
        } else {
            dep_time = curr_act.end_time;
            self.get_walk_distance(curr_act, network)
        };

        let walking_time_in_sec = distance / self.walking_speed_in_m_per_sec;

        agent.update_next_leg(
            dep_time,
            Some(walking_time_in_sec as u32),
            vec![],
            Some(distance),
            curr_act.link_id,
            next_act.link_id,
        );
    }
}
