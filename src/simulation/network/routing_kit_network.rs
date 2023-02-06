use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};

#[derive(Debug, Clone)]
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

    pub fn clone_with_new_travel_times(
        old: RoutingKitNetwork,
        travel_times: Vec<u32>,
    ) -> RoutingKitNetwork {
        let mut result = old.clone();
        result.travel_time = travel_times;
        result
    }
}
