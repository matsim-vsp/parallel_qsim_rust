use crate::simulation::replanning::routing::alt_router::ZeroHeuristic;
use crate::simulation::replanning::routing::dijsktra::{
    Dijkstra, DijkstraActions, DijkstraRequestBuilder, DijkstraResult,
};
use crate::simulation::replanning::routing::graph::NodeIdxOptions;
use crate::simulation::replanning::routing::graph::{GraphError, NodeIndex};
use crate::simulation::replanning::routing::least_cost_path_caluclator::TravelDisutility;
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    FreeSpeedTravelTimeAndDisutility, IntNodeGraph,
};
use rand::SeedableRng;
use rand::prelude::IteratorRandom;
use rand::rngs::StdRng;
use std::f64;

pub type ForwardBackwardTravelDisutility = (f64, f64);

const DEFAULT_NUMBER_OF_LANDMARKS: usize = 16;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AltLandmarkData {
    landmarks: Vec<usize>,
    travel_disutilities_to_all: Vec<Vec<ForwardBackwardTravelDisutility>>,
}

#[derive(Clone)]
pub(crate) struct DistanceToManyOptions;

impl DijkstraActions for DistanceToManyOptions {
    // fn get_parents_opt(&self) -> Option<Vec<Option<NodeIndex>>> {
    //     None
    // }
    fn set_parent_opt(&mut self, _child: NodeIndex, _parent: NodeIndex) {}
    fn reached_end(&self, _current_node: NodeIndex) -> bool {
        false
    }
    fn build_result(self, _current_distance: Option<f64>, distances: Vec<f64>) -> DijkstraResult {
        DijkstraResult::DistanceToAllWithoutParents(distances)
    }
    fn get_to_node_opt(&self) -> NodeIdxOptions {
        NodeIdxOptions::Any
    }
}

impl AltLandmarkData {
    pub fn new(graph: &Box<dyn IntNodeGraph>) -> Result<AltLandmarkData, GraphError> {
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

    fn choose_landmarks(
        graph: &Box<dyn IntNodeGraph>,
        // graph: &ForwardBackwardGraph
    ) -> Vec<NodeIndex> {
        let number_of_landmarks = if graph.num_nodes() < DEFAULT_NUMBER_OF_LANDMARKS.pow(2) {
            (graph.num_nodes() as f64 / 16.).ceil() as usize
        } else {
            DEFAULT_NUMBER_OF_LANDMARKS
        };
        //TODO do not choose random landmarks
        (0..graph.num_nodes()).choose_multiple(&mut StdRng::seed_from_u64(42), number_of_landmarks)
    }

    fn calculate_distances(
        graph: &Box<dyn IntNodeGraph>, // &ForwardBackwardGraph,
        // request: &LandmarkCreationRequest,
        landmarks: &[NodeIndex],
    ) -> Result<Vec<Vec<ForwardBackwardTravelDisutility>>, GraphError> {
        landmarks
            .iter()
            .map(
                |landmark_node| -> Result<Vec<ForwardBackwardTravelDisutility>, GraphError> {
                    let forward_distances =
                        Self::distance_one_2_many(graph, *landmark_node, false)?;
                    let backward_distances =
                        Self::distance_one_2_many(graph, *landmark_node, true)?;

                    Ok(forward_distances
                        .into_iter()
                        .zip(backward_distances.into_iter())
                        .collect::<Vec<ForwardBackwardTravelDisutility>>())

                    // Self::distance_one_2_many(*l, &graph.forward_graph, false)
                    //     .into_iter()
                    //     .zip(distance_one_2_many(*l, &graph.backward_graph, true))
                    //     .collect::<Vec<ForwardBackwardTravelDisutility>>()
                },
            )
            .collect()
    }

    /// Calculates the distance from one node to all other nodes in the graph using Dijkstra
    /// Uses the shared dijkstra_main_loop core with travel_time cost function
    /// Note: we use "distance" even though we mean travel disutility, to be consistent with the
    /// naming in Dijkstra.
    fn distance_one_2_many(
        graph: &Box<dyn IntNodeGraph>,
        from: NodeIndex,
        backward: bool, // if true, calculates distances from all other nodes to the "from"-node
    ) -> Result<Vec<f64>, GraphError> {
        let td_boxed: Box<dyn TravelDisutility> = Box::new(FreeSpeedTravelTimeAndDisutility);

        // FIXME: right now, the distance_one_2_many function never passes a vehicle to dijkstra.
        // Thus, always freespeed will be used. Previously, since the travel time was part of the graph, and was calculated for specific vehicles, this was not the case.
        // need to decide if that is what we want
        let dijkstra_request = DijkstraRequestBuilder::default()
            .graph(graph)
            .travel_disutility(&td_boxed)
            // makes Dijkstra calculate the distance to all other nodes, without tracking parents
            .options(DistanceToManyOptions)
            .from(NodeIdxOptions::One(from))
            .heuristic(&ZeroHeuristic)
            .backward(backward)
            .build()
            .unwrap();

        let distances_result = match Dijkstra::dijkstra_core(dijkstra_request) {
            Err(e) => Err(e), // some graph error occurred in dijkstra (link or node not found)
            Ok(DijkstraResult::DistanceToAllWithoutParents(distances)) => Ok(distances),
            _ => panic!(
                "dijkstra with DistanceToManyOptions should return DistanceToAllWithoutParents \
                result."
            ),
        };
        distances_result
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
    use crate::simulation::replanning::routing::graph::tests::get_triangle_test_graph;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::IntNodeGraph;

    #[test]
    // #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore // TODO still not sure if this is true
    fn test_landmark_choice() {
        let graph_boxed: Box<dyn IntNodeGraph> = Box::new(get_triangle_test_graph());
        let alt_data = AltLandmarkData::new(&graph_boxed).unwrap();

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
