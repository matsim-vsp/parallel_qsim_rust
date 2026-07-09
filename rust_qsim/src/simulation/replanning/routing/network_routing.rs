use crate::simulation::id::Id;
use crate::simulation::replanning::routing::least_cost_path_calculator::{
    LeastCostPath, LeastCostPathCalculator, LeastCostPathRequestBuilder,
};
use crate::simulation::replanning::routing::{
    RoutingModule, RoutingRequest, RoutingRequestBuilder,
};
use crate::simulation::scenario::facilities::Facility;
use crate::simulation::scenario::population::{
    InternalGenericRoute, InternalLeg, InternalNetworkRoute, InternalPlanElement, InternalRoute,
};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scenario::{ScenarioCore, network};
use crate::simulation::time::SimTime;
use crate::simulation::time::time_interpretation::TimeInterpretation;
use std::sync::Arc;

struct NetworkRoutingModule {
    mode: Id<String>,
    access_router: Arc<dyn RoutingModule>,
    egress_router: Arc<dyn RoutingModule>,
    least_cost_path_calculator: Box<dyn LeastCostPathCalculator>,
    scenario: ScenarioCore,
}

impl RoutingModule for NetworkRoutingModule {
    fn calc_route(&self, request: RoutingRequest) -> Vec<InternalPlanElement> {
        let mut result = Vec::with_capacity(5);

        // ====== route access leg
        let mut now = self.access_routing(&request, &mut result);

        // ====== route "true" leg
        now = self.network_leg(&request, now, &mut result);

        // ======= route egress leg
        self.egress_routing(&request, now, &mut result);

        result
    }
}

impl NetworkRoutingModule {
    fn access_routing(
        &self,
        request: &RoutingRequest,
        result: &mut Vec<InternalPlanElement>,
    ) -> SimTime {
        let coord = network::utils::find_nearest_point_on_link(
            request.from.coord(),
            request.from.link(),
            self.scenario.network.as_ref(),
        );
        let to = Facility::new_link_wrapper_from(request.to, coord);

        let new_req = RoutingRequestBuilder::default()
            .from(request.from)
            .to(&to)
            .departure_time(request.departure_time)
            .attributes(request.attributes.clone())
            .person(request.person)
            .build()
            .unwrap();

        let access = self.access_router.calc_route(new_req);
        let now = TimeInterpretation::decide_on_elements_end_time(&access, &request.departure_time)
            .unwrap();
        result.extend(access);
        now
    }

    fn egress_routing(
        &self,
        original_request: &RoutingRequest,
        now: SimTime,
        result: &mut Vec<InternalPlanElement>,
    ) {
        let coord = network::utils::find_nearest_point_on_link(
            original_request.to.coord(),
            original_request.to.link(),
            self.scenario.network.as_ref(),
        );
        let from = Facility::new_link_wrapper_from(original_request.to, coord);

        let new_req = RoutingRequestBuilder::default()
            .from(&from)
            .to(original_request.to)
            .departure_time(now)
            .attributes(original_request.attributes.clone())
            .person(original_request.person)
            .build()
            .unwrap();

        let egress = self.egress_router.calc_route(new_req);
        result.extend(egress);
    }

    fn network_leg(
        &self,
        request: &RoutingRequest,
        now: SimTime,
        result: &mut Vec<InternalPlanElement>,
    ) -> SimTime {
        let from = request
            .from
            .modal_link(&self.mode)
            .unwrap_or_else(|| request.from.link())
            .clone();
        let to = request
            .to
            .modal_link(&self.mode)
            .unwrap_or_else(|| request.to.link())
            .clone();
        let person = request.person;

        let r = LeastCostPathRequestBuilder::default()
            .from(from.clone())
            .to(to.clone())
            .person(person)
            .vehicle(request.vehicle)
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

        let elements = self.path_to_elements(path, &self.mode, request.vehicle);
        let time = TimeInterpretation::decide_on_elements_end_time(&elements, &now).unwrap();
        result.extend(elements);
        time
    }

    fn path_to_elements(
        &self,
        path: LeastCostPath,
        mode: &Id<String>,
        vehicle: Option<&InternalVehicle>,
    ) -> Vec<InternalPlanElement> {
        let start = path.path.first().unwrap().clone();
        let end = path.path.last().unwrap().clone();
        let time = path.travel_time;

        // calculate the distance of the path. Ignore the first link (since vehicle starts at the very end).
        let distance: f64 = path.path[1..]
            .iter()
            .map(|link| self.scenario.network.get_link(link))
            .map(|l| l.length)
            .sum();

        let generic = InternalGenericRoute::new(
            start,
            end,
            Some(time),
            Some(distance),
            vehicle.map(|v| v.id().clone()),
        );

        let net_route = InternalNetworkRoute::new(generic, path.path);
        let route = InternalRoute::Network(net_route);
        let leg = InternalLeg::new(route, mode.external(), path.travel_time, None);
        vec![InternalPlanElement::Leg(leg)]
    }
}
