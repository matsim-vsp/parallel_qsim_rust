use crate::simulation::id::Id;
use crate::simulation::replanning::routing::alt_router::{
    AStarHeuristic, AStarRouter, ZeroHeuristic,
};
use crate::simulation::replanning::routing::dijsktra::{
    Dijkstra, DijkstraActions, DijkstraRequest, DijkstraRequestBuilder, Distance,
};
use crate::simulation::replanning::routing::graph::{
    ForwardBackwardGraph, NodeIndex, RoutingGraph,
};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    IntNodeGraph, TravelDisutility,
};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{Time, TravelTime};
use crate::simulation::scenario::network::Node;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use itertools::Itertools;
use keyed_priority_queue::Entry;
use rand::SeedableRng;
use rand::prelude::IteratorRandom;
use rand::rngs::StdRng;
use std::f64;

pub type ForwardBackwardTravelTime = (u32, u32);

const DEFAULT_NUMBER_OF_LANDMARKS: usize = 16;

#[allow(dead_code)]
pub struct AltLandmarkData {
    landmarks: Vec<usize>,
    travel_times_to_all: Vec<Vec<ForwardBackwardTravelTime>>,
}

pub struct DistanceToAllRequest<'r, G: IntNodeGraph + ?Sized, T: TravelDisutility> {
    pub from: Id<Node>,
    pub graph: &'r G, // contains the graph of the network
    pub departure_time: Time,
    pub person: Option<&'r InternalPerson>,
    pub vehicle: Option<&'r InternalVehicle>,
    pub travel_disutility: T,
    pub backward: bool, // if true, calculates distance from all other nodes to the "from"-node
}

#[derive(Clone)]
pub(crate) struct DistanceToManyOptions;

impl DijkstraActions for DistanceToManyOptions {
    fn get_parents_opt(&self) -> Option<Vec<Option<NodeIndex>>> {
        None
    }
    fn set_parent_opt(&mut self, _child: NodeIndex, _parent: NodeIndex) {}
    fn reached_end(&self, _current_node: NodeIndex) -> bool {
        false
    }
}

impl AltLandmarkData {
    pub fn new(graph: &ForwardBackwardGraph) -> AltLandmarkData {
        let landmarks: Vec<usize> = Self::choose_landmarks(graph);
        let travel_times_to_all = Self::calculate_distances(graph, &landmarks);
        AltLandmarkData {
            landmarks,
            travel_times_to_all,
        }
    }

    pub fn travel_times_to_all(&self) -> &Vec<Vec<ForwardBackwardTravelTime>> {
        &self.travel_times_to_all
    }

    fn choose_landmarks(graph: &ForwardBackwardGraph) -> Vec<usize> {
        let number_of_landmarks = if graph.number_of_nodes() < DEFAULT_NUMBER_OF_LANDMARKS.pow(2) {
            (graph.number_of_nodes() as f64 / 16.).ceil() as usize
        } else {
            DEFAULT_NUMBER_OF_LANDMARKS
        };
        //TODO do not choose random landmarks
        (0..graph.number_of_nodes())
            .choose_multiple(&mut StdRng::seed_from_u64(42), number_of_landmarks)
    }

    fn calculate_distances(
        graph: &ForwardBackwardGraph,
        landmarks: &[usize],
    ) -> Vec<Vec<ForwardBackwardTravelTime>> {
        landmarks
            .iter()
            .map(|l| {
                distance_one_2_many(*l, &graph.forward_graph, false)
                    .into_iter()
                    .zip(distance_one_2_many(*l, &graph.backward_graph, true))
                    .collect::<Vec<ForwardBackwardTravelTime>>()
            })
            .collect()
    }

    /// Calculates the distance from one node to all other nodes in the graph using Dijkstra
    /// Uses the shared dijkstra_main_loop core with travel_time cost function
    fn distance_one_2_many<G: IntNodeGraph, T: TravelDisutility>(
        //from: NodeIndex, graph: impl IntNodeGraph, travel_time: Box<dyn TravelTime>, backward: bool
        request: DistanceToAllRequest<G, T>,
    ) -> Vec<f64> {
        let dijkstra_request = DijkstraRequest::from(request);

        // TODO maybe also change name "distances" here
        let (distances, _) = Dijkstra::dijkstra_core(dijkstra_request);

        // let from_idx = request.graph.get_node_idx_from_id(request.from);
        //
        // let (mut queue, mut distances) =
        //     AStarRouter::get_initial_queue(request.graph.num_nodes(), from_idx);
        //
        // while let Some((current_id, current_distance)) = queue.pop() {
        //     if current_distance.get() == f64::INFINITY || current_distance.get() == f64::NAN {
        //         //The smallest value in queue was unreachable. So abort here.
        //         return distances;
        //     }
        //
        //     // let begin_index_adjacent_nodes = graph.first_out[current_id];
        //     // let end_index_adjacent_nodes = graph.first_out[current_id + 1];
        //
        //     for i in request.graph.outgoing_edges_as_idx(current_id) {
        //         //we need an update_or_insert + parent update here instead of push always.
        //         let neighbour = request.graph.get_end_node_as_idx(i);
        //
        //         if let Entry::Vacant(_) = queue.entry(neighbour) {
        //             continue;
        //         }
        //
        //         // let link_id_i = graph.get_link_id_from_idx(i);
        //         // let link_i = graph.edge(link_id_i);
        //
        //         let link_i = request.graph.get_link_from_idx(i);
        //
        //         let travel_time_i = request.travel_time.travel_time(
        //             link_i,
        //             request.departure_time,
        //             request.person,
        //             request.vehicle,
        //         );
        //
        //         if queue.get_priority(&neighbour).unwrap().get()
        //             > current_distance.get() + travel_time_i
        //         {
        //             //perform update
        //             match queue.entry(neighbour) {
        //                 Entry::Occupied(e) => {
        //                     e.set_priority(Distance(current_distance.get() + travel_time_i));
        //                 }
        //                 Entry::Vacant(_) => {
        //                     unreachable!();
        //                 }
        //             }
        //             //store in distance vec to return
        //             distances[neighbour] = current_distance.get() + travel_time_i;
        //         }
        //     }
        // }
        distances
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
    use crate::simulation::replanning::routing::graph::tests::get_triangle_test_graph;

    #[test]
    #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore
    fn test_landmark_choice() {
        let graph = get_triangle_test_graph();
        let alt_data = AltLandmarkData::new(&graph);

        //selection so far by random seed
        assert_eq!(alt_data.landmarks.len(), 1);
        assert_eq!(alt_data.landmarks[0], 3);
        assert_eq!(
            alt_data.travel_times_to_all,
            vec![vec![(u32::MAX, u32::MAX), (2, 2), (3, 4), (0, 0)]]
        )
    }
}
