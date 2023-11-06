use rand::prelude::IteratorRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::simulation::replanning::routing::dijsktra::Dijkstra;
use crate::simulation::replanning::routing::graph::ForwardBackwardGraph;

pub type ForwardBackwardTravelTime = (u32, u32);

const DEFAULT_NUMBER_OF_LANDMARKS: usize = 16;

#[allow(dead_code)]
pub struct AltLandmarkData {
    landmarks: Vec<usize>,
    travel_times_to_all: Vec<Vec<ForwardBackwardTravelTime>>,
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
                Dijkstra::distance_one_2_many(*l, &graph.forward_graph)
                    .into_iter()
                    .zip(Dijkstra::distance_one_2_many(*l, &graph.backward_graph))
                    .collect::<Vec<ForwardBackwardTravelTime>>()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
    use crate::simulation::replanning::routing::graph::tests::get_triangle_test_graph;

    #[test]
    fn test_landmark_choice() {
        let graph = get_triangle_test_graph();
        let alt_data = AltLandmarkData::new(&graph);

        //selection so far by random seed
        assert_eq!(alt_data.landmarks.len(), 1);
        assert_eq!(alt_data.landmarks[0], 1);
        assert_eq!(
            alt_data.travel_times_to_all,
            vec![vec![(u32::MAX, u32::MAX), (0, 0), (1, 6), (2, 2)]]
        )
    }
}
