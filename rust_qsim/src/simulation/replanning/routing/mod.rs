use crate::simulation::InternalAttributes;
use crate::simulation::id::Id;
use crate::simulation::scenario::facilities::Facility;
use crate::simulation::scenario::population::{InternalPerson, InternalPlanElement};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::time::SimTime;
use derive_builder::Builder;
use nohash_hasher::IntMap;
use std::sync::Arc;

pub mod a_star;
mod a_star_core;
pub mod alt_landmark_data;
mod graph;
pub mod least_cost_path_calculator;
mod network_converter;
mod network_routing;
mod teleportation;
pub mod travel_time_collector;

pub struct TripRouter {
    modules: IntMap<Id<String>, Arc<dyn RoutingModule>>,
}

#[derive(Builder, Clone)]
#[builder(pattern = "owned")]
pub struct RoutingRequest<'r> {
    from: &'r Facility,
    to: &'r Facility,
    #[builder(default)]
    departure_time: SimTime,
    #[builder(default)]
    person: Option<&'r InternalPerson>,
    #[builder(default)]
    vehicle: Option<&'r InternalVehicle>,
    #[builder(default)]
    attributes: InternalAttributes,
}

trait RoutingModule {
    fn calc_route(&self, request: RoutingRequest) -> Vec<InternalPlanElement>;
}

struct TransitRoutingModule {}

impl RoutingModule for TransitRoutingModule {
    fn calc_route(&self, request: RoutingRequest) -> Vec<InternalPlanElement> {
        // calculate transit leg -> connect with Java router?
        todo!()
    }
}
