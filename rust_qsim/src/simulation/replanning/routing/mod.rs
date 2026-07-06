use crate::simulation::id::Id;
use crate::simulation::scenario::facility::Facility;
use crate::simulation::scenario::population::{InternalPerson, InternalPlanElement};
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

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct RoutingRequest<'r> {
    from: Facility,
    to: Facility,
    #[builder(default)]
    departure_time: SimTime,
    #[builder(default)]
    person: Option<&'r InternalPerson>,
}

trait RoutingModule {
    fn calc_route(&self, request: RoutingRequest) -> Vec<InternalPlanElement>;
}

struct NetworkRoutingModule {
    // ref to access routing module
    // ref to egress routing module
    // ref to network routing module
}

impl RoutingModule for NetworkRoutingModule {
    fn calc_route(&self, request: RoutingRequest) -> Vec<InternalPlanElement> {
        // calculate access + "normal" leg + egress
        todo!()
    }
}

struct TransitRoutingModule {}

impl RoutingModule for TransitRoutingModule {
    fn calc_route(&self, request: RoutingRequest) -> Vec<InternalPlanElement> {
        // calculate transit leg -> connect with Java router?
        todo!()
    }
}
