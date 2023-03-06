use crate::simulation::network::routing_kit_network::RoutingKitNetwork;
use crate::simulation::routing::router::{CustomQueryResult, Router};
use crate::simulation::routing::rust_road_router::RustRoadRouter;
use rust_road_router::algo::customizable_contraction_hierarchy::CCH;

pub struct RustRoadRouterAdapter<'rw> {
    cch: CCH,
    router: Option<RustRoadRouter<'rw>>,
}

impl<'rw> RustRoadRouterAdapter<'rw> {
    pub(crate) fn new(network: &RoutingKitNetwork, output_dir: &str) -> RustRoadRouterAdapter<'rw> {
        let mut instance = RustRoadRouterAdapter {
            cch: RustRoadRouter::perform_preprocessing(&network, output_dir),
            router: None,
        };
        instance.router = Some(RustRoadRouter::new(&instance.cch as *const CCH, network));
        instance
    }
}

impl<'rw> Router for RustRoadRouterAdapter<'rw> {
    fn query_coordinates(
        &mut self,
        x_from: f32,
        y_from: f32,
        x_to: f32,
        y_to: f32,
    ) -> CustomQueryResult {
        self.router
            .as_mut()
            .unwrap()
            .query_coordinates(x_from, x_to, y_from, y_to)
    }

    fn query_links(&mut self, from_link: u64, to_link: u64) -> CustomQueryResult {
        self.router
            .as_mut()
            .unwrap()
            .query_links(from_link, to_link)
    }

    fn customize(&mut self, network: RoutingKitNetwork) {
        self.router
            .as_mut()
            .unwrap()
            .customize(&self.cch as *const CCH, network);
    }

    fn get_current_network(&self) -> &RoutingKitNetwork {
        &self.router.as_ref().unwrap().current_network
    }

    fn get_initial_travel_time(&self, link_id: u64) -> u32 {
        self.router
            .as_ref()
            .unwrap()
            .initial_network
            .get_travel_time_by_link_id(link_id)
    }

    fn get_current_travel_time(&self, link_id: u64) -> u32 {
        self.router
            .as_ref()
            .unwrap()
            .current_network
            .get_travel_time_by_link_id(link_id)
    }
}
