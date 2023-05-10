use log::debug;
use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};
use std::collections::HashMap;
use kiddo::KdTree;
use kiddo::distance::squared_euclidean;

#[derive(Debug, Clone, PartialEq)]
pub struct RoutingKitNetwork {
    //CSR graph representation
    pub(crate) first_out: Vec<EdgeId>,
    pub(crate) head: Vec<NodeId>,
    pub(crate) travel_times: Vec<Weight>,
    pub(crate) link_ids: Vec<u64>,
    pub(crate) x: Vec<f32>,
    pub(crate) y: Vec<f32>,
    pub(crate) node_index: KdTree<f32, 2>,
}

impl RoutingKitNetwork {
    pub(crate) fn new(first_out: Vec<EdgeId>, head: Vec<NodeId>, travel_times: Vec<u32>, link_ids: Vec<u64>, x: Vec<f32>, y: Vec<f32>) -> RoutingKitNetwork {

        let index_entries: Vec<[f32; 2]> = x.iter().enumerate()
        .map(|(i, x_val)| {
            [*x_val, *y.get(i).unwrap()]
        }).collect();

        let tree : KdTree<_, 2> = (&index_entries).into();

        RoutingKitNetwork {
            first_out,
            head,
            travel_times,
            link_ids,
            x,
            y,
            node_index: tree
        }
    }

    pub fn find_nearest_node_id(&self, x: f32, y: f32) -> usize {

        // TODO Design: This could instead yield the closest link if we had information about
        // in and/or out links of a node. The algorithm would then find nearest node from index,
        // compute distance between center point of links and (x,y) the shortest distance would be
        // closest link. 
        let (distance, index) = self.node_index.nearest_one(&[x, y], &squared_euclidean);
        index
    }

    pub fn get_travel_time_by_link_id(&self, link_id: u64) -> u32 {
        let index = self.link_ids.iter().position(|&l| l == link_id);
        index
            .map(|i| {
                *self
                    .travel_times
                    .get(i)
                    .expect(&*format!("There is no travel time for link {:?}", link_id))
                    as u32
            })
            .unwrap()
    }

    pub(crate) fn clone_with_new_travel_times(&self, travel_times: Vec<u32>) -> RoutingKitNetwork {
        let mut result = self.clone();
        result.travel_times = travel_times;
        result
    }

    pub fn clone_with_new_travel_times_by_link(
        &self,
        new_travel_times_by_link: &HashMap<&u64, u32>,
    ) -> RoutingKitNetwork {
        let mut new_travel_time_vector = Vec::new();

        assert_eq!(self.link_ids.len(), self.travel_times.len());
        for (index, &id) in self.link_ids.iter().enumerate() {
            if let Some(new_travel_time) = new_travel_times_by_link.get(&(id as u64)) {
                new_travel_time_vector.push(*new_travel_time);
                debug!("Link {:?} | new travel time {:?}", id, new_travel_time);
            } else {
                new_travel_time_vector.push(*self.travel_times.get(index).unwrap())
            }
        }

        self.clone_with_new_travel_times(new_travel_time_vector)
    }

    pub fn has_different_travel_times(&self, other: &RoutingKitNetwork) -> bool {
        let x = self.travel_times != other.travel_times;
        if x {
            debug!(
                "New travel times are {:?}, old travel times are {:?}",
                self.travel_times, other.travel_times
            );
        }
        x
    }
}

#[cfg(test)]
mod test {
    //TODO
}
