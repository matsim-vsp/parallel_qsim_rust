use geo::{Closest, ClosestPoint, Distance, Euclidean, Line, Point};
use std::fmt::Debug;

use crate::simulation::network::global_network::Network;
use crate::simulation::wire_types::population::Activity;

pub trait TeleportedRouter {
    fn query_access_egress(
        &self,
        curr_act: &Activity,
        access_egress_speed: f32,
        network: &Network,
    ) -> Teleportation;

    fn query_between_acts(
        &self,
        curr_act: &Activity,
        next_act: &Activity,
        access_egress_speed: f32,
    ) -> Teleportation;
}

impl Debug for dyn TeleportedRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GenericRouter")
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Teleportation {
    pub distance: f64,
    pub duration: u32,
}

pub struct BeeLineDistanceRouter {}

impl Default for BeeLineDistanceRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl BeeLineDistanceRouter {
    pub fn new() -> Self {
        Self {}
    }

    fn query_points(speed: f32, p1: Point, p2: Point) -> Teleportation {
        let distance = Euclidean::distance(p1, p2);
        let duration = (distance / speed as f64) as u32;
        Teleportation { distance, duration }
    }
}

impl TeleportedRouter for BeeLineDistanceRouter {
    fn query_access_egress(
        &self,
        activity: &Activity,
        speed: f32,
        network: &Network,
    ) -> Teleportation {
        let curr_act_point = Point::new(activity.x, activity.y);
        let link = network.get_link_form_internal(activity.link_id);

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
        Self::query_points(speed, curr_act_point, closest)
    }

    fn query_between_acts(
        &self,
        curr_act: &Activity,
        next_act: &Activity,
        speed: f32,
    ) -> Teleportation {
        let curr_act_point = Point::new(curr_act.x, curr_act.y);
        let next_act_point = Point::new(next_act.x, next_act.y);

        Self::query_points(speed, curr_act_point, next_act_point)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::network::global_network::Network;
    use crate::simulation::population::population_data::Population;
    use crate::simulation::replanning::teleported_router::{
        BeeLineDistanceRouter, Teleportation, TeleportedRouter,
    };
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::wire_types::population::Person;

    #[test]
    fn test_teleported_router() {
        let network = Network::from_file(
            "./assets/equil/equil-network.xml",
            1,
            PartitionMethod::Metis(MetisOptions::default()),
        );

        let teleported_router = BeeLineDistanceRouter::new();

        let mut garage = Garage::from_file(&PathBuf::from("./assets/3-links/vehicles.xml"));
        let mut population = Population::from_file(
            &PathBuf::from("./assets/equil/equil-1-plan.xml"),
            &mut garage,
        );
        let agent = population
            .persons
            .get_mut(&Id::<Person>::get_from_ext("1"))
            .unwrap();

        // Activity(-25,000;0), Link from(-20,000;0), to(-15,000;0) => distance to link 5,000
        let walk = teleported_router.query_access_egress(agent.curr_act(), 1.2, &network);
        assert_eq!(
            walk,
            Teleportation {
                distance: 5000.,
                duration: (5000. / 1.2) as u32,
            }
        );

        agent.advance_plan();
        agent.advance_plan();

        // Activity(3,456;4,242), Link from(0;0), to(5,000;0) => distance to link 4,242
        let walk = teleported_router.query_access_egress(agent.curr_act(), 1.2, &network);
        assert_eq!(
            walk,
            Teleportation {
                distance: 4242.,
                duration: (4242. / 1.2f32) as u32,
            }
        )
    }
}
