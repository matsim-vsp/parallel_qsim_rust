use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct RoutingKitNetwork {
    //CSR graph representation
    pub(crate) first_out: Vec<EdgeId>,
    pub(crate) head: Vec<NodeId>,
    pub(crate) travel_time: Vec<Weight>,
    pub(crate) link_ids: Vec<usize>,
    pub(crate) latitude: Vec<f32>,
    pub(crate) longitude: Vec<f32>,
}

impl RoutingKitNetwork {
    pub(crate) fn new() -> RoutingKitNetwork {
        RoutingKitNetwork {
            first_out: Vec::new(),
            head: Vec::new(),
            travel_time: Vec::new(),
            link_ids: Vec::new(),
            latitude: Vec::new(),
            longitude: Vec::new(),
        }
    }

    pub fn clone_with_new_travel_times(&self, travel_times: Vec<u32>) -> RoutingKitNetwork {
        let mut result = self.clone();
        result.travel_time = travel_times;
        result
    }

    pub fn clone_with_new_travel_times_if_changes_present(
        &self,
        new_travel_times_by_link: HashMap<&u64, &u32>,
    ) -> Option<RoutingKitNetwork> {
        let mut new_travel_time_vector = Vec::new();

        assert_eq!(self.link_ids.len(), self.travel_time.len());
        let mut changed_travel_time_flag = false;
        for (&id, &old_travel_time) in self.link_ids.iter().zip(self.travel_time.iter()) {
            match new_travel_times_by_link.get(&(id as u64)) {
                None => {
                    new_travel_time_vector.push(old_travel_time);
                }
                Some(&new_travel_time) => {
                    new_travel_time_vector.push(*new_travel_time);
                    //if flag is true once, it stays true
                    changed_travel_time_flag |= *new_travel_time != old_travel_time;
                }
            }
        }

        match changed_travel_time_flag {
            false => None,
            true => Some(self.clone_with_new_travel_times(new_travel_time_vector)),
        }
    }
}

#[cfg(test)]
mod test {
    //TODO
}
