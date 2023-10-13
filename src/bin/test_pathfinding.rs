fn main() {}

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
}

pub struct Graph {
    pub first_out: Vec<usize>,
    pub head_out: Vec<usize>,
    pub travel_time_out: Vec<u32>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            first_out: vec![],
            head_out: vec![],
            travel_time_out: vec![],
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ResultPath {
    distance: u32,
    path: Vec<usize>,
}

//To work properly we expect for n nodes to have ids from 0 to n-1.
pub fn dijkstra_path(from: usize, to: usize, graph: Graph) -> Option<ResultPath> {
    let mut distances: Vec<u32> = (0..graph.first_out.len() - 1).map(|_| u32::MAX).collect();
    let mut parents: Vec<Option<usize>> = (0..graph.first_out.len() - 1).map(|_| None).collect();
    let mut traversed: Vec<bool> = (0..graph.first_out.len() - 1).map(|_| false).collect();

    //update start node
    distances[from] = 0;

    while let Some(current) = get_next_node(&mut distances, &mut traversed) {
        let current_id = current.0;
        let current_distance = distances[current_id];

        if current_id == to {
            return Some(ResultPath {
                distance: current_distance,
                path: extract_path(to, parents),
            });
        }

        let begin_index_adjacent_nodes = graph.first_out[current_id];
        let end_index_adjacent_nodes = graph.first_out[current_id + 1];

        for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
            //we need an update_or_insert + parent update here instead of push always.
            let neighbour = graph.head_out[i];

            if traversed[neighbour] {
                continue;
            }

            if distances[neighbour] > current_distance + graph.travel_time_out[i] {
                //perform update
                distances[neighbour] = current_distance + graph.travel_time_out[i];
                parents[neighbour] = Some(current_id);
            }
        }
        traversed[current_id] = true;
    }
    None
}

pub fn dijkstra_distances(from: usize, graph: Graph) -> Vec<u32> {
    let mut distances: Vec<u32> = (0..graph.first_out.len() - 1).map(|_| u32::MAX).collect();
    let mut traversed: Vec<bool> = (0..graph.first_out.len() - 1).map(|_| false).collect();

    //update start node
    distances[from] = 0;

    while let Some(current) = get_next_node(&mut distances, &mut traversed) {
        let current_id = current.0;
        let current_distance = distances[current_id];

        let begin_index_adjacent_nodes = graph.first_out[current_id];
        let end_index_adjacent_nodes = graph.first_out[current_id + 1];

        for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
            //we need an update_or_insert + parent update here instead of push always.
            let neighbour = graph.head_out[i];

            if traversed[neighbour] {
                continue;
            }

            if distances[neighbour] > current_distance + graph.travel_time_out[i] {
                //perform update
                distances[neighbour] = current_distance + graph.travel_time_out[i];
            }
        }
        traversed[current_id] = true;
    }
    distances
}

//To work properly we expect for n nodes to have ids from 0 to n-1.
pub fn alt_path(
    from: usize,
    to: usize,
    graph: Graph,
    landmark_distances: Vec<Vec<u32>>,
) -> Option<ResultPath> {
    let mut distances: Vec<u32> = (0..graph.first_out.len() - 1).map(|_| u32::MAX).collect();
    let mut f_score: Vec<u32> = (0..graph.first_out.len() - 1).map(|_| u32::MAX).collect();
    let mut parents: Vec<Option<usize>> = (0..graph.first_out.len() - 1).map(|_| None).collect();

    let mut traversed: Vec<bool> = (0..graph.first_out.len() - 1).map(|_| false).collect();

    //update start node
    f_score[from] = 0;
    distances[from] = 0;

    while let Some(current) = get_next_node(&mut f_score, &mut traversed) {
        let current_id = current.0;
        let current_distance = distances[current_id];

        if current_id == to {
            return Some(ResultPath {
                distance: current_distance,
                path: extract_path(to, parents),
            });
        }

        let begin_index_adjacent_nodes = graph.first_out[current_id];
        let end_index_adjacent_nodes = graph.first_out[current_id + 1];

        for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
            //we need an update_or_insert + parent update here instead of push always.
            let neighbour = graph.head_out[i];

            if traversed[neighbour] {
                continue;
            }

            let neighbour_distance = current_distance + graph.travel_time_out[i];

            if distances[neighbour] > neighbour_distance {
                //perform update
                distances[neighbour] = neighbour_distance;
                f_score[neighbour] =
                    neighbour_distance + heuristic(neighbour, to, &landmark_distances);
                parents[neighbour] = Some(current_id);
            }
        }
        traversed[current_id] = true;
    }
    None
}

fn heuristic(node: usize, goal: usize, landmark_distances: &Vec<Vec<u32>>) -> u32 {
    //TODO implement backward search
    let mut h = 0;
    for l in landmark_distances {
        h = h.max(l[node] as i32 - l[goal] as i32)
    }
    if h < 0 {
        0
    } else {
        h as u32
    }
}

fn get_next_node<'a>(
    distances: &'a Vec<u32>,
    traversed: &'a Vec<bool>,
) -> Option<(usize, (&'a u32, &'a bool))> {
    //ToDo: Never pick a node with distance == u32::MAX
    distances
        .iter()
        .zip(traversed.iter())
        .enumerate()
        .filter(|(_, (_, &t))| !t)
        .min_by(|a, b| a.1 .0.cmp(b.1 .0))
}

fn extract_path(to: usize, parent: Vec<Option<usize>>) -> Vec<usize> {
    let mut path = Vec::new();
    let mut current = to;

    path.push(to);
    while let Some(father) = parent[current] {
        path.push(father);
        current = father;
    }

    path.reverse();
    path
}

#[cfg(test)]
mod test {
    use crate::{alt_path, dijkstra_distances, dijkstra_path, Graph, ResultPath};

    fn create_network() -> Graph {
        Graph {
            first_out: vec![0, 2, 4, 5],
            head_out: vec![1, 2, 1, 2, 0],
            travel_time_out: vec![1, 2, 1, 4, 2],
        }
    }

    #[test]
    fn test_simple_dijkstra() {
        let option = dijkstra_path(2, 1, create_network());
        println!("{:?}", option);
        assert_eq!(
            option,
            Some(ResultPath {
                distance: 3,
                path: vec![2, 0, 1],
            })
        )
    }

    #[test]
    fn test_simple_dijkstra2() {
        let option = dijkstra_path(0, 2, create_network());
        println!("{:?}", option);
        assert_eq!(
            option,
            Some(ResultPath {
                distance: 2,
                path: vec![0, 2],
            })
        )
    }

    #[test]
    fn test_distances_from0() {
        let result = dijkstra_distances(0, create_network());
        println!("{:?}", result);
        assert_eq!(result, vec![0, 1, 2])
    }

    #[test]
    fn test_distances_from1() {
        let result = dijkstra_distances(1, create_network());
        println!("{:?}", result);
        assert_eq!(result, vec![6, 0, 4])
    }

    #[test]
    fn test_distances_from2() {
        let result = dijkstra_distances(2, create_network());
        println!("{:?}", result);
        assert_eq!(result, vec![2, 3, 0])
    }

    #[test]
    fn test_alt() {
        let mut preprocessed_landmark = Vec::new();
        preprocessed_landmark.push(dijkstra_distances(0, create_network()));

        let result = alt_path(2, 1, create_network(), preprocessed_landmark);
        println!("{:?}", result);
        assert_eq!(
            result,
            Some(ResultPath {
                distance: 3,
                path: vec![2, 0, 1],
            })
        );
    }

    #[test]
    fn test_alt2() {
        let mut preprocessed_landmark = Vec::new();
        preprocessed_landmark.push(dijkstra_distances(0, create_network()));
        let result = alt_path(0, 2, create_network(), preprocessed_landmark);
        println!("{:?}", result);
        assert_eq!(
            result,
            Some(ResultPath {
                distance: 2,
                path: vec![0, 2],
            })
        )
    }
}
