use crate::simulation::routing::graph::ForwardBackwardGraph;
use crate::simulation::routing::router::CustomQueryResult;

pub struct AltRouter {
    pub landmarks: Vec<u64>,
    pub landmarks_distances: Vec<Vec<u32>>,
    pub current_graph: ForwardBackwardGraph,
    pub initial_graph: ForwardBackwardGraph,
}

struct AltQueryResult {
    travel_time: Option<u32>,
    node_path: Option<Vec<usize>>,
}

impl AltQueryResult {
    fn empty() -> Self {
        Self {
            travel_time: None,
            node_path: None,
        }
    }

    fn node_path(self) -> Option<Vec<usize>> {
        self.node_path
    }
}

impl AltRouter {
    pub fn empty() -> Self {
        AltRouter {
            landmarks: vec![],
            landmarks_distances: vec![],
            current_graph: ForwardBackwardGraph::empty(),
            initial_graph: ForwardBackwardGraph::empty(),
        }
    }

    pub fn new(graph: ForwardBackwardGraph) -> Self {
        //TODO run preprocessing here.
        AltRouter {
            landmarks: vec![],
            landmarks_distances: vec![],
            current_graph: graph.clone(),
            initial_graph: graph,
        }
    }

    fn query(&mut self, from: usize, to: usize) -> AltQueryResult {
        //TODO run ALT algorithm here
        AltQueryResult::empty()
    }

    pub fn query_links(&mut self, from_link: u64, to_link: u64) -> CustomQueryResult {
        let travel_time;
        let result_edge_path;
        {
            let mut result = self.query(self.get_end_node(from_link), self.get_start_node(to_link));
            travel_time = result.travel_time;
            result_edge_path = result.node_path();
        }
        let edge_path = result_edge_path
            .map(|node_path| Self::get_edge_path(node_path, &self.current_graph))
            .map(|mut path| {
                //add from link at the beginning and to link at the end
                path.insert(0, from_link);
                path.push(to_link);
                path
            });

        CustomQueryResult {
            travel_time,
            path: edge_path,
        }
    }

    fn get_end_node(&self, link_id: u64) -> usize {
        let link_id_index = self
            .current_graph
            .forward_link_ids()
            .iter()
            .position(|&id| id == link_id as usize)
            .unwrap();
        *self
            .current_graph
            .forward_head()
            .get(link_id_index)
            .unwrap() as usize
    }

    fn get_start_node(&self, link_id: u64) -> usize {
        let link_id_index = self
            .current_graph
            .forward_link_ids()
            .iter()
            .position(|&id| id == link_id as usize)
            .unwrap();

        let mut result = None;
        for i in 0..self.current_graph.forward_first_out().len() {
            if link_id_index >= *self.current_graph.forward_first_out().get(i).unwrap() as usize
                && link_id_index
                    < *self.current_graph.forward_first_out().get(i + 1).unwrap() as usize
            {
                result = Some(i as usize);
            }
        }

        result.unwrap()
    }

    fn perform_preprocessing(&mut self) {}

    pub fn customize(&mut self) {}

    pub fn get_initial_travel_time(&self, link_id: u64) -> u32 {
        self.initial_graph.get_travel_time_by_link_id(link_id)
    }

    pub fn get_current_travel_time(&self, link_id: u64) -> u32 {
        self.current_graph.get_travel_time_by_link_id(link_id)
    }

    fn get_edge_path(path: Vec<usize>, graph: &ForwardBackwardGraph) -> Vec<u64> {
        let mut res = Vec::new();
        let mut last_node: Option<usize> = None;
        for node in path {
            match last_node {
                None => last_node = Some(node as usize),
                Some(n) => {
                    let first_out_index = *graph.forward_first_out().get(n).unwrap() as usize;
                    let last_out_index =
                        (graph.forward_first_out().get(n + 1).unwrap() - 1) as usize;
                    res.push(Self::find_edge_id_of_outgoing(
                        first_out_index,
                        last_out_index,
                        node,
                        graph,
                    ));
                    last_node = Some(node as usize)
                }
            }
        }
        res
    }

    fn find_edge_id_of_outgoing(
        first_out_index: usize,
        last_out_index: usize,
        next_node: usize,
        graph: &ForwardBackwardGraph,
    ) -> u64 {
        assert!(
            last_out_index as i64 - first_out_index as i64 >= 0,
            "No outgoing edges!"
        );
        let mut result = None;
        for i in first_out_index..=last_out_index {
            if *graph.forward_head().get(i).unwrap() == next_node {
                result = Some(graph.forward_link_ids().get(i).unwrap().clone());
                break;
            }
        }
        result.expect("No outgoing edge found!") as u64
    }
}
