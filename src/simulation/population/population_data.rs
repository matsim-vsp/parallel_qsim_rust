use crate::simulation::id::Id;
use crate::simulation::network::global_network::Network;
use crate::simulation::population::io::{from_file, to_file};
use crate::simulation::population::InternalPerson;
use crate::simulation::time_queue::{EndTime, Identifiable};
use crate::simulation::vehicles::garage::Garage;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

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
//
//     pub fn id(&self) -> u64 {
//         self.id
//     }
//
//     pub fn curr_act(&self) -> &Activity {
//         if self.curr_plan_elem % 2 != 0 {
//             panic!("Current element is not an activity");
//         }
//         let act_index = self.curr_plan_elem / 2;
//         self.get_act_at_index(act_index)
//     }
//
//     pub fn curr_leg(&self) -> &Leg {
//         if self.curr_plan_elem % 2 != 1 {
//             panic!("Current element is not a leg.");
//         }
//
//         let leg_index = (self.curr_plan_elem - 1) / 2;
//         self.plan
//             .as_ref()
//             .unwrap()
//             .legs
//             .get(leg_index as usize)
//             .unwrap()
//     }
//
//     pub fn next_leg(&self) -> Option<&Leg> {
//         // position index: 0      | 1
//         // activities:     a0 (0) | a1 (2)
//         // legs:           l0 (1) | l1 (3)
//         // e.g., if current is a1, next leg is l1 => curr_plan_elem/2
//         // e.g., if current is l0, next leg is l1 => (curr_plan_elem + 1)/2
//
//         let next_leg_index = if self.curr_plan_elem % 2 == 0 {
//             // current element is an activity
//             self.curr_plan_elem / 2
//         } else {
//             // current element is a leg
//             (self.curr_plan_elem + 1) / 2
//         };
//
//         self.plan
//             .as_ref()
//             .unwrap()
//             .legs
//             .get(next_leg_index as usize)
//     }
//
//     fn get_act_at_index(&self, index: u32) -> &Activity {
//         self.plan
//             .as_ref()
//             .unwrap()
//             .acts
//             .get(index as usize)
//             .unwrap()
//     }
//
//     pub fn advance_plan(&mut self) {
//         let next = self.curr_plan_elem + 1;
//         if self.plan.as_ref().unwrap().acts.len() + self.plan.as_ref().unwrap().legs.len()
//             == next as usize
//         {
//             panic!(
//                 "Person: Advance plan was called on Person #{}, but no element is remaining.",
//                 self.id
//             )
//         }
//         self.curr_plan_elem = next;
//     }
//
//     pub fn legs(&self) -> &[Leg] {
//         self.plan.as_ref().unwrap().legs.as_slice()
//     }
//
//     pub fn acts(&self) -> &[Activity] {
//         self.plan.as_ref().unwrap().acts.as_slice()
//     }
// }
//
// impl EndTime for Person {
//     fn end_time(&self, now: u32) -> u32 {
//         if self.curr_plan_elem % 2 == 0 {
//             self.curr_act().cmp_end_time(now)
//         } else {
//             self.curr_leg().trav_time + now
//         }
//     }
// }
//
// impl Identifiable for Person {
//     fn id(&self) -> u64 {
//         self.id
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
//
//     pub fn add_leg(&mut self, leg: Leg) {
//         self.legs.push(leg);
//     }
//
//     pub fn add_act(&mut self, activity: Activity) {
//         self.acts.push(activity);
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
//
//     pub fn is_interaction(&self) -> bool {
//         Id::<String>::get(self.act_type)
//             .external()
//             .contains("interaction")
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
//     pub fn as_generic(&self) -> &GenericRoute {
//         match self {
//             Route::GenericRoute(g) => g,
//             Route::NetworkRoute(n) => n.delegate.as_ref().unwrap(),
//             Route::PtRoute(p) => p.delegate.as_ref().unwrap(),
//         }
//     }
//
//     pub fn as_network(&self) -> Option<&NetworkRoute> {
//         match self {
//             Route::NetworkRoute(n) => Some(n),
//             _ => None,
//         }
//     }
//
//     pub fn as_pt(&self) -> Option<&PtRoute> {
//         match self {
//             Route::PtRoute(p) => Some(p),
//             _ => None,
//         }
//     }
//
//     pub fn start_link(&self) -> u64 {
//         self.as_generic().start_link
//     }
//
//     pub fn end_link(&self) -> u64 {
//         self.as_generic().end_link
//     }
//
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

// fn trim_quotes(s: &Value) -> String {
//     s.to_string().trim_matches('"').to_string()
// }
//
// fn parse_time_opt(value: &Option<String>) -> Option<u32> {
//     if let Some(time) = value.as_ref() {
//         parse_time(time)
//     } else {
//         None
//     }
// }
//
// fn parse_time(value: &str) -> Option<u32> {
//     let split: Vec<&str> = value.split(':').collect();
//     if split.len() == 3 {
//         let hour: u32 = split.first().unwrap().parse().unwrap();
//         let minutes: u32 = split.get(1).unwrap().parse().unwrap();
//         let seconds: u32 = split.get(2).unwrap().parse().unwrap();
//
//         Some(hour * 3600 + minutes * 60 + seconds)
//     } else {
//         None
//     }
// }

#[derive(Debug, Default, PartialEq)]
pub struct Population {
    pub persons: HashMap<Id<InternalPerson>, InternalPerson>,
}

impl Population {
    pub fn new() -> Self {
        Population {
            persons: HashMap::default(),
        }
    }

    pub fn from_file(file_path: &Path, garage: &mut Garage) -> Self {
        from_file(file_path, garage, |_p| true)
    }

    pub fn from_file_filtered<F>(file_path: &Path, garage: &mut Garage, filter: F) -> Self
    where
        F: Fn(&InternalPerson) -> bool,
    {
        from_file(file_path, garage, filter)
    }

    pub fn from_file_filtered_part(
        file_path: &Path,
        net: &Network,
        garage: &mut Garage,
        part: u32,
    ) -> Self {
        from_file(file_path, garage, |p| {
            let act = p.plan_element_at(0).as_activity().unwrap();
            let partition = net.get_link(&act.link_id).partition;
            partition == part
        })
    }

    pub fn to_file(&self, file_path: &Path) {
        to_file(self, file_path);
    }
}

#[cfg(test)]
mod tests {}
