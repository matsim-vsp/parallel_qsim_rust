use crate::generated::vehicles::Vehicle;
use crate::scenario::network::Link;
use crate::simulation::id::Id;
use crate::simulation::replanning::routing::alt_router::{
    AStarHeuristic, AStarRouter, NodePriority,
};
use crate::simulation::replanning::routing::graph::{LinkIndex, NodeIndex, RoutingGraph};
use crate::simulation::replanning::routing::least_cost_path_caluclator::Time;
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    IntNodeGraph, TravelDisutility,
};
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use keyed_priority_queue::{Entry, KeyedPriorityQueue};
use std::cmp::Ordering;

#[deprecated] // should use OrderedFloat, which is simply a float64 modified to implement Eq
pub struct Distance(pub f64);

// we have to implement PartialEq manually for Distance, since we need the Eq trait, i.e.,
// reflexivity. Therefore, we treat two NaN distances as equal (while in general f64::NaN != f64::NaN)
impl PartialEq for Distance {
    fn eq(&self, other: &Self) -> bool {
        match (self.0.is_nan(), other.0.is_nan()) {
            (true, true) => true,                // both values NaN -> equal
            (true, false) => false,              // left value NaN -> not equal
            (false, true) => false,              // right value NaN -> not equal
            (false, false) => self.0 == other.0, // compare normally
        }
    }
}

impl Eq for Distance {}

impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0) // None if one of the values is NaN
    }
}

impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(
                // both values NaN -> equal
                if self.0.is_nan() && other.0.is_nan() {
                    Ordering::Equal
                }
                // left value NaN -> Greater, since NaN is bad, i.e., large distance
                else if self.0.is_nan() {
                    Ordering::Greater
                }
                // right value NaN -> Less, since NaN is bad, i.e., large distance
                else {
                    Ordering::Less
                },
            )
            .reverse() // reverse, since priority queue prefers large values
    } // FIXME remove reverse and change the priority queue function instead
} // TODO probably remove the entire Distance struct and use some crate

impl Distance {
    pub fn get(&self) -> f64 {
        self.0
    }
}

pub(crate) trait DijkstraActions {
    fn reached_end(&self, current_node: NodeIndex) -> bool;
    fn set_parent_opt(&mut self, child: NodeIndex, parent: NodeIndex);
    fn get_parents_opt(&self) -> Option<Vec<Option<NodeIndex>>>;
}

#[deprecated]
// I think this is not needed any more. The functions associated with it should go into the A Star module.
// However, the A Star might need a Dijkstra implementation.
pub struct Dijkstra {}

impl Dijkstra {
    /// calculates the distance from one node to all other nodes in the graph (Dikstra)
    pub(crate) fn distance_one_2_many(from: usize, graph: &RoutingGraph) -> Vec<u32> {
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
    // TODO put parameters into a request struct
    // TODO use builder (derive(builder))
    pub(crate) fn dijkstra_core<
        H: AStarHeuristic,
        T: TravelDisutility, // TODO use this again
        D: Fn(&Link) -> f64,
        G: IntNodeGraph + Sized,
        O: DijkstraActions,
    >(
        heuristic: H,
        // travel_disutility: T,
        distance_per_link: D,
        from: Id<Link>,
        to: Option<Id<Link>>,
        graph: &G,
        options: O,
        // departure_time: Time, // TODO set default to 0
        // person: Option<&InternalPerson>, // TODO set default to none in builder
        // vehicle: Option<&InternalVehicle>,
    ) -> (Vec<f64>, Option<Vec<Option<NodeIndex>>>) {
        let number_of_nodes = graph.num_nodes();

        let from_node = graph.get_node_idx_from_id(graph.get_end_node(from));
        let to_node = graph.get_node_idx_from_id(graph.get_start_node(to));

        // TODO possibly rename distances to priorities?
        // TODO is it reasonable to take the distances separately as f64? wouldn't it be nicer to extract them from the queue?
        // check why is it even done in this way
        let (mut queue, mut distances) = AStarRouter::get_initial_queue(number_of_nodes, from_node);
        // Not initializing parents here, since they are contained in the options
        while let Some((current_id, _)) = queue.pop() {
            let current_distance = distances[current_id];

            if current_distance == f64::INFINITY || current_distance == f64::NAN {
                // TODO do we want this?
                //The smallest value in queue was unreachable. So abort here.
                return (distances, None);
            }

            if options.reached_end(current_id) == true {
                return (distances, options.get_parents_opt()); //TODO maybe return something else
            }

            for i in graph.outgoing_edges_as_idx(current_id) {
                //we need an update_or_insert + parent update here instead of push always.

                // let neighbour = request.graph.forward_graph.head[i];
                let neighbour = graph.get_end_node_as_idx(i);

                if let Entry::Vacant(_) = queue.entry(neighbour) {
                    continue;
                }

                let link_i = graph.get_link_from_idx(i);

                // TODO is it correct to use the departure time from the request here? -> NO!
                // or could it be later by now?
                let neighbour_distance = current_distance
                    // FIXME this is where we should use travel disutility and not the other fct
                    + distance_per_link(link_i);
                // + travel_disutility.travel_disutility(
                // link_i,
                // departure_time,
                // person,
                // vehicle,
                // ); // (request.graph.forward_graph.travel_time[i] as f64);

                if distances[neighbour] > neighbour_distance {
                    //perform update
                    distances[neighbour] = neighbour_distance;

                    match queue.entry(neighbour) {
                        Entry::Occupied(e) => {
                            e.set_priority(NodePriority::new(
                                neighbour_distance
                                    + heuristic.estimate(
                                        graph,
                                        graph.get_node_id_from_idx(neighbour),
                                        graph.get_node_id_from_idx(to_node),
                                    ), // TODO remove when sure that not needed: &self.landmark_data),
                            ));
                        }
                        Entry::Vacant(_) => {
                            unreachable!()
                        }
                    }
                    options.set_parent_opt(neighbour, current_id);
                    //     TODO need to make sure (all) distances are tracked as well
                }
            }
        }
        return (distances, options.get_parents_opt());
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
