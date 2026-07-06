use crate::simulation::id::Id;
use crate::simulation::replanning::routing::{RoutingModule, RoutingRequest};
use crate::simulation::scenario::Coordinate;
use crate::simulation::scenario::population::{
    InternalGenericRoute, InternalLeg, InternalPlanElement, InternalRoute,
};
use std::time::Duration;

struct TeleportationRoutingModule {
    mode: Id<String>,
    beeline_distance_factor: f64,
    travel_speed: f64,
}

impl RoutingModule for TeleportationRoutingModule {
    fn calc_route(&self, request: RoutingRequest) -> Vec<InternalPlanElement> {
        let mode = self.mode.external();
        let dep_time = Some(request.departure_time);

        let start = request.from.modal_link_id(&self.mode).unwrap_or_else(|| {
            panic!(
                "Teleportation routing from facility {} requires a link id for mode {}.",
                request.from.id(),
                self.mode
            )
        });
        let end = request.to.modal_link_id(&self.mode).unwrap_or_else(|| {
            panic!(
                "Teleportation routing to facility {} requires a link id for mode {}.",
                request.to.id(),
                self.mode
            )
        });

        let from_coord = request.from.coord().unwrap_or_else(|| {
            panic!(
                "Teleportation routing from facility {} requires coordinates.",
                request.from.id()
            )
        });
        let to_coord = request.to.coord().unwrap_or_else(|| {
            panic!(
                "Teleportation routing to facility {} requires coordinates.",
                request.to.id()
            )
        });

        let distance =
            Coordinate::euclidean_distance(from_coord, to_coord) * self.beeline_distance_factor;

        let trav_time = Duration::from_secs_f64(distance * self.travel_speed);
        let route = InternalRoute::Generic(InternalGenericRoute::new(
            start,
            end,
            Some(trav_time),
            Some(distance),
            None,
        ));

        let leg = InternalLeg::new(route, mode, trav_time, dep_time);
        vec![InternalPlanElement::Leg(leg)]
    }
}
