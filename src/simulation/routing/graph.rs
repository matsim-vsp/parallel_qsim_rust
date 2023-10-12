pub struct AltGraph {
    pub forward_graph: Graph,
    pub backward_graph: Graph,
}

impl AltGraph {
    pub fn new(forward_graph: Graph, backward_graph: Graph) -> Self {
        Self {
            forward_graph,
            backward_graph,
        }
    }

    pub fn empty() -> Self {
        Self {
            forward_graph: Graph::new(),
            backward_graph: Graph::new(),
        }
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
}

pub struct Graph {
    first_out: Vec<usize>,
    head: Vec<usize>,
    travel_time: Vec<u32>,
    link_ids: Vec<u64>,
    x: Vec<f32>,
    y: Vec<f32>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            first_out: vec![],
            head: vec![],
            travel_time: vec![],
            link_ids: vec![],
            x: vec![],
            y: vec![],
        }
    }
}