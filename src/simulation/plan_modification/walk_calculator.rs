use geo::{Closest, ClosestPoint, EuclideanDistance, Line, Point};

use crate::simulation::messaging::messages::proto::Activity;
use crate::simulation::network::global_network::Network;

pub trait WalkCalculator {
    fn find_walk(&self, curr_act: &Activity, network: &Network) -> Walk;
}

pub struct Walk {
    pub distance: f32,
    pub duration: u32,
}

pub struct EuclideanWalkCalculator {
    walking_speed_in_m_per_sec: f32,
}

impl EuclideanWalkCalculator {
    pub fn new(walking_speed_in_m_per_sec: f32) -> Self {
        Self {
            walking_speed_in_m_per_sec,
        }
    }
}

impl WalkCalculator for EuclideanWalkCalculator {
    fn find_walk(&self, curr_act: &Activity, network: &Network) -> Walk {
        let curr_act_point = Point::new(curr_act.x, curr_act.y);
        //TODO is it correct?
        let link = network
            .links
            .iter()
            .find(|l| l.id.internal() == curr_act.link_id)
            .unwrap();

        let from_node_id = &link.from;
        let to_node_id = &link.to;

        let from_node_x = network.get_node(from_node_id).x;
        let from_node_y = network.get_node(from_node_id).y;

        let to_node_x = network.get_node(to_node_id).x;
        let to_node_y = network.get_node(to_node_id).y;

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
        let distance = curr_act_point.euclidean_distance(&closest);
        let duration = (distance / self.walking_speed_in_m_per_sec) as u32;
        Walk { distance, duration }
    }
}
