#[derive(Clone, Debug)]
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

    pub fn empty() -> Self {
        Self {
            forward_graph: Graph::new(),
            backward_graph: Graph::new(),
        }
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
            self.forward_graph.first_out.len(),
            self.backward_graph.first_out.len()
        );
    }

    pub fn get_travel_time_by_link_id(&self, link_id: u64) -> u32 {
        let index = self
            .forward_link_ids()
            .iter()
            .position(|&l| l == link_id as usize);
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

    pub fn forward_link_ids(&self) -> &Vec<usize> {
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
}

#[derive(Clone, Debug)]
pub struct Graph {
    pub(crate) first_out: Vec<usize>,
    pub(crate) head: Vec<usize>,
    pub(crate) travel_time: Vec<u32>,
    pub(crate) link_ids: Vec<usize>,
    pub(crate) x: Vec<f32>,
    pub(crate) y: Vec<f32>,
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
