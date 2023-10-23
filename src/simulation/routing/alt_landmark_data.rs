use rand::prelude::IteratorRandom;
use rand::thread_rng;

use crate::simulation::routing::graph::{ForwardBackwardGraph, Graph};

pub type ForwardBackwardTravelTime = (u32, u32);

const DEFAULT_NUMBER_OF_LANDMARKS: usize = 16;

#[allow(dead_code)]
pub struct AltLandmarkData {
    landmarks: Vec<usize>,
    travel_times_to_all: Vec<Vec<ForwardBackwardTravelTime>>,
}

impl AltLandmarkData {
    pub fn new(graph: &ForwardBackwardGraph) -> AltLandmarkData {
        let landmarks: Vec<usize> = Self::choose_landmarks(&graph);
        let travel_times_to_all = Self::calculate_distances(&graph, &landmarks);
        AltLandmarkData {
            landmarks,
            travel_times_to_all,
        }
    }

    pub fn travel_times_to_all(&self) -> &Vec<Vec<ForwardBackwardTravelTime>> {
        &self.travel_times_to_all
    }

    fn choose_landmarks(graph: &ForwardBackwardGraph) -> Vec<usize> {
        let number_of_landmarks =
            if graph.number_of_nodes() < DEFAULT_NUMBER_OF_LANDMARKS.pow(2) as usize {
                (graph.number_of_nodes() as f64 / 16.).ceil() as usize
            } else {
                DEFAULT_NUMBER_OF_LANDMARKS
            };
        //TODO do not choose random landmarks
        (0..graph.number_of_nodes()).choose_multiple(&mut thread_rng(), number_of_landmarks)
    }

    fn calculate_distances(
        graph: &ForwardBackwardGraph,
        landmarks: &Vec<usize>,
    ) -> Vec<Vec<ForwardBackwardTravelTime>> {
        landmarks
            .iter()
            .map(|l| {
                Self::dijkstra_distances(*l, &graph.forward_graph)
                    .into_iter()
                    .zip(Self::dijkstra_distances(*l, &graph.backward_graph))
                    .collect::<Vec<(u32, u32)>>()
            })
            .collect()
    }
    fn dijkstra_distances(from: usize, graph: &Graph) -> Vec<u32> {
        let mut distances: Vec<u32> = (0..graph.first_out.len() - 1).map(|_| u32::MAX).collect();
        let mut traversed: Vec<bool> = (0..graph.first_out.len() - 1).map(|_| false).collect();

        //update start node
        distances[from] = 0;

        while let Some(current) = Self::get_next_node(&mut distances, &mut traversed) {
            let current_id = current.0;
            let current_distance = distances[current_id];

            let begin_index_adjacent_nodes = graph.first_out[current_id];
            let end_index_adjacent_nodes = graph.first_out[current_id + 1];

            for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
                //we need an update_or_insert + parent update here instead of push always.
                let neighbour = graph.head[i];

                if traversed[neighbour] {
                    continue;
                }

                if distances[neighbour] > current_distance + graph.travel_time[i] {
                    //perform update
                    distances[neighbour] = current_distance + graph.travel_time[i];
                }
            }
            traversed[current_id] = true;
        }
        distances
    }

    fn get_next_node<'a>(
        travel_times: &'a Vec<u32>,
        traversed: &'a Vec<bool>,
    ) -> Option<(usize, (&'a u32, &'a bool))> {
        let result = travel_times
            .iter()
            .zip(traversed.iter())
            .enumerate()
            .filter(|(_, (_, &t))| !t)
            .min_by(|a, b| a.1 .0.cmp(b.1 .0));

        if result.is_none() {
            return None;
        }

        if result.map(|(_, (t, _))| t).unwrap() >= &u32::MAX {
            return None;
        }

        result
    }
}
