use crate::simulation::replanning::routing::a_star_core::{
    AStarCoreResult, AStarRequestBuilder, HeuristicMode, One2ManyNoParentsAStarActions, a_star_core,
};
use crate::simulation::replanning::routing::graph::{GraphError, IndexableGraph, NodeIndex};
use crate::simulation::replanning::routing::least_cost_path_calculator::{
    Disutility, FreeSpeedTravelTimeAndDisutility,
};
use rand::SeedableRng;
use rand::prelude::IteratorRandom;
use rand::rngs::StdRng;
use std::f64;

pub type ForwardBackwardTravelDisutility = (Disutility, Disutility);

const DEFAULT_NUMBER_OF_LANDMARKS: usize = 16;

/// Landmark data to be used in ALT routing. Contains the chosen landmarks and the pre-calculated
/// distances from each landmark to all other nodes in the graph, for both forward and backward
/// directions.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AltLandmarkData {
    landmarks: Vec<NodeIndex>,
    travel_disutilities_to_all: Vec<Vec<ForwardBackwardTravelDisutility>>,
}

impl AltLandmarkData {
    /// Given a graph, chooses landmarks (currently randomly) and precalculates their distances,
    /// i.e., travel disutilities, to all other nodes, both forward and backward
    pub fn new(graph: &dyn IndexableGraph) -> Result<AltLandmarkData, GraphError> {
        let landmarks: Vec<NodeIndex> = Self::choose_landmarks(graph);

        let travel_disutilities_to_all = Self::calculate_distances(graph, &landmarks)?;

        Ok(AltLandmarkData {
            landmarks,
            travel_disutilities_to_all,
        })
    }

    pub fn travel_disutilities_to_all(&self) -> &Vec<Vec<ForwardBackwardTravelDisutility>> {
        &self.travel_disutilities_to_all
    }

    fn choose_landmarks(graph: &dyn IndexableGraph) -> Vec<NodeIndex> {
        let number_of_landmarks = if graph.num_nodes() < DEFAULT_NUMBER_OF_LANDMARKS.pow(2) {
            (graph.num_nodes() as f64 / 16.).ceil() as usize
        } else {
            DEFAULT_NUMBER_OF_LANDMARKS
        };
        //TODO do not choose random landmarks
        (0..graph.num_nodes()).choose_multiple(&mut StdRng::seed_from_u64(42), number_of_landmarks)
    }

    /// Calculate distances from given list of landmarks to all other nodes in the graph, both
    /// forward and backward.
    fn calculate_distances(
        graph: &dyn IndexableGraph,
        landmarks: &[NodeIndex],
    ) -> Result<Vec<Vec<ForwardBackwardTravelDisutility>>, GraphError> {
        // for every landmark...
        landmarks
            .iter()
            .map(
                |landmark_node| -> Result<Vec<ForwardBackwardTravelDisutility>, GraphError> {
                    // ... calculate forward and backward distances to all other nodes
                    let forward_distances =
                        Self::distance_one_2_many(graph, *landmark_node, false)?;
                    let backward_distances =
                        Self::distance_one_2_many(graph, *landmark_node, true)?;

                    // collect into ForwardBackwardTravelDisutility objects
                    Ok(forward_distances
                        .into_iter()
                        .zip(backward_distances.into_iter())
                        .collect::<Vec<ForwardBackwardTravelDisutility>>())
                },
            )
            .collect()
    }

    /// Calculates the distance from one node to all other nodes in the graph using Dijkstra.
    /// "Distance" in this case means travel disutility, which is in this case specifically equal
    /// to the freespeed travel time, *not* respecting max speed of any vehicle.  TODO is this what we want? see below
    /// Uses the shared `a_star_core` implementation with `ZeroHeuristic` and the One2Many use case.
    fn distance_one_2_many(
        graph: &dyn IndexableGraph,
        from: NodeIndex,
        backward: bool, // if true, calculates distances from all other nodes to the "from"-node
    ) -> Result<Vec<Disutility>, GraphError> {
        // TODO: right now, the distance_one_2_many uses FreeSpeedTravelTimeAndDisutility, not FreeOrMaxSpeed...
        // Thus, always freespeed will be used. Previously, since the travel time was part of the
        // graph, and was calculated for specific vehicles, this was not the case; the max speed of
        // vehicles was respected.
        // Need to decide if that is what we want, or if the landmarks should be valid for any
        // vehicle as they are now
        let tt_td = FreeSpeedTravelTimeAndDisutility;

        let a_star_request = AStarRequestBuilder::default()
            .graph(graph)
            // set travel time and disutility function to freespeed
            .travel_time(&tt_td)
            .travel_disutility(&tt_td)
            // makes A* calculate the distance to all other nodes, without tracking parents
            .options(One2ManyNoParentsAStarActions)
            .from(from)
            // no heuristic used => A* is Dijkstra
            .heuristic_mode(HeuristicMode::without_heuristic())
            .backward(backward)
            .build()
            .unwrap();

        let distances_result = match a_star_core(a_star_request) {
            // some graph error occurred in A* (link or node not found). Return it.
            Err(e) => Err(e),
            // everything fine, A* returned a distance vector; use it
            Ok(AStarCoreResult::DistanceToAllWithoutParents(distances)) => Ok(distances),
            // A* returned incorrect result enum variant. Panic, since this is a programming error.
            _ => panic!(
                "A* with DistanceToManyOptions should return DistanceToAllWithoutParents \
                result."
            ),
        };

        distances_result
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
    use crate::simulation::replanning::routing::graph::tests::{
        get_triangle_test_graph, get_triangle_test_network,
    };

    #[test]
    // #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore // TODO still not sure if this is true
    fn test_landmark_choice_and_distance_calculation() {
        let network = get_triangle_test_network();
        let graph = get_triangle_test_graph(&network);

        let alt_data = AltLandmarkData::new(&graph).unwrap();

        //selection is so far random, but with fixed seed (chooses node with index 3 as landmark)

        // verify that exactly one landmark was chosen, namely node with index 3
        assert_eq!(alt_data.landmarks.len(), 1);
        assert_eq!(alt_data.landmarks[0], 3);
        // verify the travel disutilities explicitly
        assert_eq!(
            alt_data.travel_disutilities_to_all,
            vec![vec![
                (f64::INFINITY, f64::INFINITY),
                (2.0, 2.0),
                (3.0, 4.0),
                (0.0, 0.0)
            ]]
        )
    }
}
