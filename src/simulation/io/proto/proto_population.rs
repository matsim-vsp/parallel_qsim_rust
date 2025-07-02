use crate::simulation::io::proto::population::leg::Route;
use crate::simulation::io::proto::population::{
    Activity, GenericRoute, Leg, NetworkRoute, Person, Plan, PtRoute, PtRouteDescription,
};
use crate::simulation::population::{
    InternalActivity, InternalGenericRoute, InternalLeg, InternalNetworkRoute, InternalPerson,
    InternalPlan, InternalPtRoute, InternalPtRouteDescription, InternalRoute,
};

impl Person {
    pub fn from(value: &InternalPerson) -> Self {
        Self {
            id: value.id().external().to_string(),
            plan: value.plans().iter().map(|p| Plan::from(p)).collect(),
            attributes: Default::default(),
        }
    }
}

impl Plan {
    fn from(value: &InternalPlan) -> Self {
        Self {
            selected: value.selected,
            legs: value.legs().iter().map(|p| Leg::from(p)).collect(),
            acts: value.acts().iter().map(|l| Activity::from(l)).collect(),
        }
    }
}

impl Activity {
    fn from(value: &InternalActivity) -> Self {
        Self {
            act_type: value.act_type.external().to_string(),
            link_id: value.link_id.external().to_string(),
            x: value.x,
            y: value.y,
            start_time: value.start_time,
            end_time: value.end_time,
            max_dur: value.max_dur,
        }
    }
}

impl Leg {
    fn from(value: &InternalLeg) -> Self {
        Self {
            mode: value.mode.external().to_string(),
            routing_mode: value
                .routing_mode
                .as_ref()
                .map(|r| r.external().to_string()),
            dep_time: value.dep_time,
            trav_time: value.trav_time,
            attributes: value.attributes.as_cloned_map(),
            route: value.route.as_ref().map(Route::from),
        }
    }
}

impl Route {
    fn from(value: &InternalRoute) -> Self {
        match value {
            InternalRoute::Generic(g) => Route::GenericRoute(GenericRoute::from(g)),
            InternalRoute::Network(n) => Route::NetworkRoute(NetworkRoute::from(n)),
            InternalRoute::Pt(p) => Route::PtRoute(PtRoute::from(p)),
        }
    }
}

impl GenericRoute {
    fn from(value: &InternalGenericRoute) -> Self {
        Self {
            start_link: value.start_link().external().to_string(),
            end_link: value.end_link().external().to_string(),
            trav_time: value.trav_time(),
            distance: value.distance(),
            veh_id: value.vehicle().as_ref().map(|v| v.external().to_string()),
        }
    }
}

impl NetworkRoute {
    fn from(value: &InternalNetworkRoute) -> Self {
        Self {
            delegate: Some(GenericRoute::from(value.generic_delegate())),
            route: value
                .route()
                .iter()
                .map(|id| id.external().to_string())
                .collect(),
        }
    }
}

impl PtRoute {
    fn from(value: &InternalPtRoute) -> Self {
        Self {
            delegate: Some(GenericRoute::from(value.generic_delegate())),
            information: Some(PtRouteDescription::from(value.description())),
        }
    }
}

impl PtRouteDescription {
    fn from(value: &InternalPtRouteDescription) -> Self {
        Self {
            transit_route_id: value.transit_route_id.clone(),
            boarding_time: value.boarding_time.clone(),
            transit_line_id: value.transit_line_id.clone(),
            access_facility_id: value.access_facility_id.clone(),
            egress_facility_id: value.egress_facility_id.clone(),
        }
    }
}

// impl Person {
//     pub fn from_io(io_person: IOPerson) -> Person {
//         let mut attributes = HashMap::new();
//         if let Some(attrs) = io_person.attributes {
//             for attr in attrs.attributes {
//                 attributes.insert(attr.name.clone(), AttributeValue::from_io_attr(attr));
//             }
//         }
//
//         let person_id = Id::get_from_ext(&io_person.id);
//
//         let selected_plan = io_person.plans.iter().find(|p| p.selected).unwrap();
//         let plan = Plan::from_io(selected_plan, &person_id);
//
//         if plan.acts.is_empty() {
//             debug!("There is an empty plan for person {:?}", io_person.id);
//         }
//
//         Person {
//             id: person_id.internal(),
//             plan: Some(plan),
//             curr_plan_elem: 0,
//             attributes,
//         }
//     }
//
//     pub fn new(id: u64, plan: Plan) -> Self {
//         Person {
//             id,
//             curr_plan_elem: 0,
//             plan: Some(plan),
//             attributes: HashMap::new(),
//         }
//     }
// }
//
// impl Plan {
//     pub fn new() -> Plan {
//         Plan {
//             acts: Vec::new(),
//             legs: Vec::new(),
//         }
//     }
//
//     fn from_io(io_plan: &IOPlan, person_id: &Id<Person>) -> Plan {
//         assert!(!io_plan.elements.is_empty());
//         if let IOPlanElement::Leg(_leg) = io_plan.elements.first().unwrap() {
//             panic!("First plan element must be an activity! But was a leg.");
//         };
//
//         let mut result = Plan::new();
//
//         for element in &io_plan.elements {
//             match element {
//                 IOPlanElement::Activity(io_act) => {
//                     let act = Activity::from_io(io_act);
//                     result.acts.push(act);
//                 }
//                 IOPlanElement::Leg(io_leg) => {
//                     let leg = Leg::from_io(io_leg, person_id);
//                     result.legs.push(leg);
//                 }
//             }
//         }
//
//         if result.acts.len() - result.legs.len() != 1 {
//             panic!("Plan {:?} has less legs than expected", io_plan);
//         }
//
//         result
//     }
// }
//
// impl Activity {
//     fn from_io(io_act: &IOActivity) -> Self {
//         let link_id: Id<Link> = Id::get_from_ext(&io_act.link);
//         let act_type: Id<String> = Id::get_from_ext(&io_act.r#type);
//         Activity {
//             x: io_act.x,
//             y: io_act.y,
//             act_type: act_type.internal(),
//             link_id: link_id.internal(),
//             start_time: parse_time_opt(&io_act.start_time),
//             end_time: parse_time_opt(&io_act.end_time),
//             max_dur: parse_time_opt(&io_act.max_dur),
//         }
//     }
//
//     pub fn new(
//         x: f64,
//         y: f64,
//         act_type: u64,
//         link_id: u64,
//         start_time: Option<u32>,
//         end_time: Option<u32>,
//         max_dur: Option<u32>,
//     ) -> Self {
//         Activity {
//             x,
//             y,
//             act_type,
//             link_id,
//             start_time,
//             end_time,
//             max_dur,
//         }
//     }
//
//     pub(crate) fn cmp_end_time(&self, now: u32) -> u32 {
//         if let Some(end_time) = self.end_time {
//             end_time
//         } else if let Some(max_dur) = self.max_dur {
//             now + max_dur
//         } else {
//             // supposed to be an equivalent for OptionalTime.undefined() in the java code
//             u32::MAX
//         }
//     }
// }
//
// impl Leg {
//     fn from_io(io_leg: &IOLeg, person_id: &Id<Person>) -> Self {
//         let routing_mode_ext = Attrs::find_or_else_opt(&io_leg.attributes, "routingMode", || "car");
//
//         let routing_mode: Id<String> = Id::create(routing_mode_ext);
//         let mode = Id::get_from_ext(io_leg.mode.as_str());
//
//         let route = io_leg
//             .route
//             .as_ref()
//             .map(|r| Route::from_io(r, person_id, &mode));
//
//         assert!(
//             route.is_some(),
//             "Route is expected to be set. This is not the case for person {} with IOLeg {:?}",
//             person_id.external(),
//             io_leg
//         );
//
//         Self {
//             route,
//             mode: mode.internal(),
//             trav_time: Self::parse_trav_time(
//                 &io_leg.trav_time,
//                 &io_leg.route.as_ref().and_then(|r| r.trav_time.clone()),
//             ),
//             dep_time: parse_time_opt(&io_leg.dep_time),
//             routing_mode: routing_mode.internal(),
//             attributes: HashMap::new(),
//         }
//     }
//
//     pub fn new(route: Route, mode: u64, trav_time: u32, dep_time: Option<u32>) -> Self {
//         Self {
//             route: Some(route),
//             mode,
//             trav_time,
//             dep_time,
//             routing_mode: 0,
//             attributes: HashMap::new(),
//         }
//     }
//
//     fn parse_trav_time(leg_trav_time: &Option<String>, route_trav_time: &Option<String>) -> u32 {
//         if let Some(trav_time) = parse_time_opt(leg_trav_time) {
//             trav_time
//         } else {
//             parse_time_opt(route_trav_time).unwrap_or(0)
//         }
//     }
// }
//
// impl Route {
//     fn from_io(io_route: &IORoute, person_id: &Id<Person>, mode: &Id<String>) -> Self {
//         let route = match io_route.r#type.as_str() {
//             "generic" => Self::from_io_generic(io_route, person_id, mode),
//             "links" => Self::from_io_net_route(io_route, person_id, mode),
//             "default_pt" => Self::from_io_pt_route(io_route, person_id, mode),
//             _t => panic!("Unsupported route type: '{_t}'"),
//         };
//         route
//     }
//
//     fn from_io_generic(io_route: &IORoute, person_id: &Id<Person>, mode: &Id<String>) -> Self {
//         let start_link: Id<Link> = Id::get_from_ext(&io_route.start_link);
//         let end_link: Id<Link> = Id::get_from_ext(&io_route.end_link);
//         let external = format!("{}_{}", person_id.external(), mode.external());
//         let veh_id: Id<Vehicle> = Id::get_from_ext(&external);
//
//         Route::GenericRoute(GenericRoute {
//             start_link: start_link.internal(),
//             end_link: end_link.internal(),
//             trav_time: io_route
//                 .trav_time
//                 .as_ref()
//                 .and_then(|t| parse_time(&t))
//                 .and_then(|t| Some(t as u64)),
//             distance: Some(io_route.distance),
//             veh_id: Some(veh_id.internal()),
//         })
//     }
//
//     fn from_io_net_route(io_route: &IORoute, person_id: &Id<Person>, mode: &Id<String>) -> Self {
//         if let Some(veh_id_ext) = &io_route.vehicle {
//             // catch this special case because we have "null" as vehicle ids for modes which are
//             // routed but not simulated on the network.
//             if veh_id_ext.eq("null") {
//                 Self::from_io_generic(io_route, person_id, mode)
//             } else {
//                 let veh_id: Id<Vehicle> = Id::get_from_ext(veh_id_ext.as_str());
//                 let link_ids = match &io_route.route {
//                     None => Vec::new(),
//                     Some(encoded_links) => encoded_links
//                         .split(' ')
//                         .map(|matsim_id| Id::<Link>::get_from_ext(matsim_id).internal())
//                         .collect(),
//                 };
//
//                 Route::NetworkRoute(NetworkRoute {
//                     delegate: Some(GenericRoute {
//                         start_link: *link_ids.first().unwrap(),
//                         end_link: *link_ids.last().unwrap(),
//                         trav_time: io_route
//                             .trav_time
//                             .as_ref()
//                             .and_then(|t| parse_time(&t))
//                             .map(|t| t as u64),
//                         distance: Some(io_route.distance),
//                         veh_id: Some(veh_id.internal()),
//                     }),
//                     route: link_ids,
//                 })
//             }
//         } else {
//             panic!("vehicle id is expected to be set.")
//         }
//     }
//
//     fn from_io_pt_route(io_route: &IORoute, person_id: &Id<Person>, mode: &Id<String>) -> Self {
//         let start_link: Id<Link> = Id::get_from_ext(&io_route.start_link);
//         let end_link: Id<Link> = Id::get_from_ext(&io_route.end_link);
//         let external = format!("{}_{}", person_id.external(), mode.external());
//         let veh_id: Id<Vehicle> = Id::get_from_ext(&external);
//
//         Route::PtRoute(PtRoute {
//             delegate: Some(GenericRoute {
//                 start_link: start_link.internal(),
//                 end_link: end_link.internal(),
//                 trav_time: io_route
//                     .trav_time
//                     .as_ref()
//                     .and_then(|t| parse_time(&t))
//                     .map(|t| t as u64),
//                 distance: Some(io_route.distance),
//                 veh_id: Some(veh_id.internal()),
//             }),
//             information: io_route
//                 .route
//                 .as_ref()
//                 .and_then(|r| PtRouteDescription::from_str(&r).ok()),
//         })
//     }
// }
//
// impl FromStr for PtRouteDescription {
//     type Err = Error;
//
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         let desc: Value = serde_json::from_str(s)?;
//
//         Ok(PtRouteDescription {
//             transit_route_id: trim_quotes(&desc["transitRouteId"]),
//             boarding_time: desc["boardingTime"].as_str().and_then(parse_time),
//             transit_line_id: trim_quotes(&desc["transitLineId"]),
//             access_facility_id: trim_quotes(&desc["accessFacilityId"]),
//             egress_facility_id: trim_quotes(&desc["egressFacilityId"]),
//         })
//     }
// }
