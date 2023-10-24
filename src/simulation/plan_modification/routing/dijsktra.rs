use crate::simulation::plan_modification::routing::graph::Graph;

pub struct Dijkstra {}

impl Dijkstra {
    pub(crate) fn distance_one_2_many(from: usize, graph: &Graph) -> Vec<u32> {
        let mut distances: Vec<u32> = (0..graph.first_out.len() - 1).map(|_| u32::MAX).collect();
        let mut traversed: Vec<bool> = (0..graph.first_out.len() - 1).map(|_| false).collect();

        //update start node
        distances[from] = 0;

        while let Some(current) = Dijkstra::get_next_node(&mut distances, &mut traversed) {
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

    pub(crate) fn get_next_node<'a>(
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
