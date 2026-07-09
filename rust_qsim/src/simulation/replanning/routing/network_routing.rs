use crate::simulation::id::Id;
use crate::simulation::replanning::routing::least_cost_path_calculator::{
    LeastCostPath, LeastCostPathCalculator, LeastCostPathRequest, LeastCostPathRequestBuilder,
};
use crate::simulation::replanning::routing::{RoutingModule, RoutingRequest};
use crate::simulation::scenario::facilities::Facility;
use crate::simulation::scenario::population::{
    InternalGenericRoute, InternalLeg, InternalNetworkRoute, InternalPlanElement, InternalRoute,
};
use crate::simulation::scenario::vehicles::Garage;
use crate::simulation::time::SimTime;
use crate::simulation::time::time_interpretation::TimeInterpretation;
use std::sync::Arc;

struct NetworkRoutingModule {
    mode: Id<String>,
    access_router: Arc<dyn RoutingModule>,
    egress_router: Arc<dyn RoutingModule>,
    least_cost_path_calculator: Box<dyn LeastCostPathCalculator>,
    garage: Arc<Garage>,
}

impl RoutingModule for NetworkRoutingModule {
    fn calc_route(&self, request: RoutingRequest) -> Vec<InternalPlanElement> {
        let mut result = Vec::with_capacity(5);

        // ====== route access leg
        let mut now = self.access_routing(&request, &mut result);

        // ====== route "true" leg
        let from = request.from.modal_link_id(&self.mode);
        let to = request.to.modal_link_id(&self.mode);
        let person = request.person;

        // set vehicle correctly
        // let vehicle = self.garage.vehicles.get()
        let vehicle = None;

        let r = LeastCostPathRequestBuilder::default()
            .from(from.clone())
            .to(to.clone())
            .person(person)
            .vehicle(vehicle)
            .departure_time(now)
            .build()
            .unwrap();

        let path = self
            .least_cost_path_calculator
            .calc_least_cost_path(r)
            .unwrap_or_else(|| {
                panic!(
                    "No route found from {} to {} with mode {} for person {:?} at time {}",
                    from,
                    to,
                    self.mode.external(),
                    person.map(|p| p.id()),
                    now
                )
            });

        let elements = path_to_elements(path, &self.mode);
        now = TimeInterpretation::decide_on_elements_end_time(&elements, now).unwrap();
        result.extend(elements);

        // ======= route egress leg

        let mut egress_request = request.clone();
        egress_request.departure_time = now;
        let egress = self.egress_router.calc_route(egress_request);

        result
    }
}

impl NetworkRoutingModule {
    fn access_routing(
        &self,
        request: &RoutingRequest,
        result: &mut Vec<InternalPlanElement>,
    ) -> SimTime {
        // TODO change "to" in the request
        let access = self.access_router.calc_route(request.clone());
        let now = TimeInterpretation::decide_on_elements_end_time(&access, request.departure_time)
            .unwrap();
        result.extend(access);
        now
    }
}

fn path_to_elements(path: LeastCostPath, mode: &Id<String>) -> Vec<InternalPlanElement> {
    let start = path.path.first().unwrap().clone();
    let end = path.path.last().unwrap().clone();
    let time = path.travel_time;

    //TODO set vehicle
    //TODO set distance
    let generic = InternalGenericRoute::new(start, end, Some(time), None, None);

    let net_route = InternalNetworkRoute::new(generic, path.path);
    let route = InternalRoute::Network(net_route);
    let leg = InternalLeg::new(route, mode.external(), path.travel_time, None);
    vec![InternalPlanElement::Leg(leg)]
}
