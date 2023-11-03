use crate::simulation::replanning::routing::graph::Graph;
use keyed_priority_queue::{Entry, KeyedPriorityQueue};
use std::cmp::Ordering;

#[derive(Eq, PartialEq)]
pub struct Distance(pub u32);

impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0).reverse()
    }
}

impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0).map(|o| o.reverse())
    }
}

impl Distance {
    pub fn get(&self) -> u32 {
        self.0
    }
}

pub struct Dijkstra {}

impl Dijkstra {
    pub(crate) fn distance_one_2_many(from: usize, graph: &Graph) -> Vec<u32> {
        let (mut queue, mut distances) =
            Dijkstra::get_initial_queue(graph.first_out.len() - 1, from);

        while let Some((current_id, current_distance)) = queue.pop() {
            if current_distance.get() == u32::MAX {
                //The smallest value in queue was unreachable. So abort here.
                return distances;
            }

            let begin_index_adjacent_nodes = graph.first_out[current_id];
            let end_index_adjacent_nodes = graph.first_out[current_id + 1];

            for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
                //we need an update_or_insert + parent update here instead of push always.
                let neighbour = graph.head[i];

                if let Entry::Vacant(_) = queue.entry(neighbour) {
                    continue;
                }

                if queue.get_priority(&neighbour).unwrap().get()
                    > current_distance.get() + graph.travel_time[i]
                {
                    //perform update
                    match queue.entry(neighbour) {
                        Entry::Occupied(e) => {
                            e.set_priority(Distance(current_distance.get() + graph.travel_time[i]));
                        }
                        Entry::Vacant(_) => {
                            unreachable!();
                        }
                    }
                    //store in distance vec to return
                    distances[neighbour] = current_distance.get() + graph.travel_time[i];
                }
            }
        }
        distances
    }

    pub fn get_initial_queue(
        node_count: usize,
        from: usize,
    ) -> (KeyedPriorityQueue<usize, Distance>, Vec<u32>) {
        let mut queue = KeyedPriorityQueue::new();
        let mut distances = Vec::new();
        for i in 0..node_count {
            let distance = if i == from {
                //update start node
                Distance(0)
            } else {
                Distance(u32::MAX)
            };
            distances.push(distance.0);
            queue.push(i, distance);
        }
        (queue, distances)
    }
}
