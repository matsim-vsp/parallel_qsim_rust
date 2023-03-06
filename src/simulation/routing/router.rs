use crate::simulation::network::routing_kit_network::RoutingKitNetwork;

pub trait Router {
    fn query_coordinates(
        &mut self,
        x_from: f32,
        y_from: f32,
        x_to: f32,
        y_to: f32,
    ) -> CustomQueryResult;

    fn query_links(&mut self, from_link: u64, to_link: u64) -> CustomQueryResult;

    fn customize(&mut self, network: RoutingKitNetwork);

    fn get_current_network(&self) -> &RoutingKitNetwork;

    fn get_initial_travel_time(&self, link_id: u64) -> u32;

    fn get_current_travel_time(&self, link_id: u64) -> u32;
}

pub struct CustomQueryResult {
    pub travel_time: Option<u32>,
    pub path: Option<Vec<u64>>,
}
