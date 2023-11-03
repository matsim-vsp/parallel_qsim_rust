use geo::{Closest, ClosestPoint, EuclideanDistance, Line, Point};

use crate::simulation::messaging::messages::proto::Activity;
use crate::simulation::network::global_network::Network;

pub trait WalkFinder {
    fn find_walk(&self, curr_act: &Activity, network: &Network, access_egress_speed: f32) -> Walk;
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Walk {
    pub distance: f64,
    pub duration: u32,
}

pub struct EuclideanWalkFinder {}

impl EuclideanWalkFinder {
    pub fn new() -> Self {
        Self {}
    }
}

impl WalkFinder for EuclideanWalkFinder {
    fn find_walk(&self, curr_act: &Activity, network: &Network, access_egress_speed: f32) -> Walk {
        let curr_act_point = Point::new(curr_act.x, curr_act.y);
        let link = network.get_link_form_internal(curr_act.link_id);

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
        let duration = (distance / access_egress_speed as f64) as u32;
        Walk { distance, duration }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::network::global_network::Network;
    use crate::simulation::replanning::walk_finder::{
        EuclideanWalkFinder, Walk, WalkFinder,
    };
    use crate::simulation::population::population::Population;
    use crate::simulation::vehicles::garage::Garage;

    #[test]
    fn test_walk_finder() {
        let walk_finder = EuclideanWalkFinder::new();

        let mut network = Network::from_file("./assets/equil/equil-network.xml", 1, "metis");
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml", &mut network.modes);
        let population =
            Population::from_file("./assets/equil/equil-1-plan.xml", &network, &mut garage, 0);
        let agent = population.agents.get(&population.agent_ids.get(0)).unwrap();

        // Activity(-25,000;0), Link from(-20,000;0), to(-15,000;0) => distance to link 5,000
        let walk = walk_finder.find_walk(agent.curr_act(), &network, 1.2);
        assert_eq!(
            walk,
            Walk {
                distance: 5000.,
                duration: (5000. / 1.2) as u32,
            }
        );

        // Activity(3,456;4,242), Link from(0;0), to(5,000;0) => distance to link 4,242
        let walk = walk_finder.find_walk(agent.next_act(), &network, 1.2);
        assert_eq!(
            walk,
            Walk {
                distance: 4242.,
                duration: (4242. / 1.2 as f32) as u32,
            }
        )
    }
}
