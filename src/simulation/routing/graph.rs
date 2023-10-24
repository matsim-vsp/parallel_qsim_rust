use std::collections::HashMap;

use clap::Parser;
use tracing::debug;

#[derive(Clone, Debug, PartialEq)]
pub struct ForwardBackwardGraph {
    pub forward_graph: Graph,
    pub backward_graph: Graph,
}

impl ForwardBackwardGraph {
    pub fn new(forward_graph: Graph, backward_graph: Graph) -> Self {
        let graph = Self {
            forward_graph,
            backward_graph,
        };
        graph.validate_else_panic();
        graph
    }

    fn validate_else_panic(&self) {
        assert_eq!(
            self.forward_graph.head.len(),
            self.backward_graph.head.len()
        );
        assert_eq!(
            self.forward_graph.travel_time.len(),
            self.backward_graph.travel_time.len()
        );
        assert_eq!(
            self.forward_graph.head.len(),
            self.backward_graph.travel_time.len()
        );
        assert_eq!(
            self.forward_graph.first_out.len(),
            self.backward_graph.first_out.len()
        );
    }

    pub fn get_forward_travel_time_by_link_id(&self, link_id: u64) -> u32 {
        let index = self.forward_link_ids().iter().position(|&l| l == link_id);
        index
            .map(|i| {
                *self
                    .forward_travel_time()
                    .get(i)
                    .expect(&*format!("There is no travel time for link {:?}", link_id))
                    as u32
            })
            .unwrap()
    }
    pub fn forward_first_out(&self) -> &Vec<usize> {
        &self.forward_graph.first_out
    }

    pub fn forward_head(&self) -> &Vec<usize> {
        &self.forward_graph.head
    }

    pub fn forward_travel_time(&self) -> &Vec<u32> {
        &self.forward_graph.travel_time
    }

    pub fn forward_link_ids(&self) -> &Vec<u64> {
        &self.forward_graph.link_ids
    }

    pub fn forward_x(&self) -> &Vec<f32> {
        &self.forward_graph.x
    }

    pub fn forward_y(&self) -> &Vec<f32> {
        &self.forward_graph.y
    }

    pub fn number_of_nodes(&self) -> usize {
        self.forward_graph.first_out.len()
    }

    pub fn clone_with_new_travel_times_by_link(
        &self,
        new_travel_times_by_link: HashMap<&u64, &u32>,
    ) -> ForwardBackwardGraph {
        ForwardBackwardGraph {
            forward_graph: self
                .forward_graph
                .clone_with_new_travel_times_by_link(&new_travel_times_by_link),
            backward_graph: self
                .backward_graph
                .clone_with_new_travel_times_by_link(&new_travel_times_by_link),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub struct Graph {
    pub(crate) first_out: Vec<usize>,
    pub(crate) head: Vec<usize>,
    pub(crate) travel_time: Vec<u32>,
    pub(crate) link_ids: Vec<u64>,
    pub(crate) x: Vec<f32>,
    pub(crate) y: Vec<f32>,
}

impl Graph {
    fn new(first_out: Vec<usize>, head: Vec<usize>, travel_time: Vec<u32>) -> Graph {
        Graph {
            first_out,
            head,
            travel_time,
            link_ids: vec![],
            x: vec![],
            y: vec![],
        }
    }

    pub fn clone_with_new_travel_times_by_link(
        &self,
        new_travel_times_by_link: &HashMap<&u64, &u32>,
    ) -> Graph {
        assert_eq!(self.link_ids.len(), self.travel_time.len());

        let mut new_travel_time_vector = Vec::new();
        for (index, &id) in self.link_ids.iter().enumerate() {
            if let Some(&&new_travel_time) = new_travel_times_by_link.get(&(id as u64)) {
                new_travel_time_vector.push(new_travel_time);
                debug!("Link {:?} | new travel time {:?}", id, new_travel_time);
            } else {
                new_travel_time_vector.push(*self.travel_time.get(index).unwrap())
            }
        }

        self.clone_with_new_travel_times(new_travel_time_vector)
    }

    fn clone_with_new_travel_times(&self, travel_times: Vec<u32>) -> Graph {
        let mut result = self.clone();
        result.travel_time = travel_times;
        result
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;

    use crate::simulation::network::global_network::Network;
    use crate::simulation::routing::graph::{ForwardBackwardGraph, Graph};
    use crate::simulation::routing::network_converter::NetworkConverter;

    pub fn get_triangle_test_graph() -> ForwardBackwardGraph {
        let network = Network::from_file("./assets/routing_tests/triangle-network.xml", 1, "metis");
        NetworkConverter::convert_network(&network, None, None)
    }

    #[test]
    #[should_panic]
    fn test_graph_not_valid() {
        let fbg = ForwardBackwardGraph::new(
            Graph::new(
                vec![0, 1, 2],
                vec![0, 1, 2, 3, 4, 5],
                vec![1, 1, 1, 1, 1, 1],
            ),
            Graph::new(vec![0, 1, 2], vec![0, 1, 2, 3, 4], vec![1, 1, 1, 1, 1]),
        );
    }

    #[test]
    fn test_graph_valid() {
        let fbg = ForwardBackwardGraph::new(
            Graph::new(
                vec![0, 1, 2],
                vec![0, 1, 2, 3, 4, 5],
                vec![1, 1, 1, 1, 1, 1],
            ),
            Graph::new(
                vec![42, 43, 44],
                vec![8, 10, 12, 13, 14, 15],
                vec![1, 1, 1, 1, 1, 10],
            ),
        );
    }

    #[test]
    fn clone_without_change() {
        let graph = get_triangle_test_graph();
        let new_graph = graph.clone_with_new_travel_times_by_link(HashMap::new());

        assert_eq!(graph, new_graph);
    }

    #[test]
    fn clone_with_change() {
        let mut graph = get_triangle_test_graph();
        let mut change = HashMap::new();
        change.insert(&5, &42);
        let new_graph = graph.clone_with_new_travel_times_by_link(change);

        //change manually
        graph.forward_graph.travel_time[5] = 42;
        graph.backward_graph.travel_time[3] = 42;
        assert_eq!(graph, new_graph);
    }
}
