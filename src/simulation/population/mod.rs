use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::network::global_network::Link;
use crate::simulation::population::io::{IOPerson, IOPlan, IOPlanElement, IORoute};
use crate::simulation::vehicles::InternalVehicle;
use crate::simulation::InternalAttributes;
use serde_json::Value;

pub mod agent_source;
pub mod io;
pub mod population_data;

#[derive(Debug, PartialEq, Clone)]
pub struct InternalActivity {
    pub act_type: Id<String>,
    pub link_id: Id<Link>,
    pub x: f64,
    pub y: f64,
    pub start_time: Option<u32>,
    pub end_time: Option<u32>,
    pub max_dur: Option<u32>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalLeg {
    pub mode: Id<String>,
    pub dep_time: Option<u32>,
    pub trav_time: Option<u32>,
    pub route: Option<InternalRoute>,
    pub attributes: Option<InternalAttributes>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum InternalRoute {
    Generic(InternalGenericRoute),
    Network(InternalNetworkRoute),
    Pt(InternalPtRoute),
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalGenericRoute {
    pub start_link: Id<Link>,
    pub end_link: Id<Link>,
    pub trav_time: Option<u32>,
    pub distance: Option<f64>,
    pub vehicle: Option<Id<InternalVehicle>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalNetworkRoute {
    pub generic_delegate: InternalGenericRoute,
    pub route: Vec<Id<Link>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalPtRoute {
    pub generic_delegate: InternalGenericRoute,
    pub description: InternalPtRouteDescription,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalPtRouteDescription {
    pub transit_route_id: String,
    pub boarding_time: Option<u32>,
    pub transit_line_id: String,
    pub access_facility_id: String,
    pub egress_facility_id: String,
}

#[derive(Debug, PartialEq, Clone)]
pub enum InternalPlanElement {
    Activity(InternalActivity),
    Leg(InternalLeg),
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalPlan {
    pub selected: bool,
    pub elements: Vec<InternalPlanElement>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalPerson {
    pub id: Id<InternalPerson>,
    pub plans: Vec<InternalPlan>,
    pub attributes: Option<Attrs>,
}

#[derive(Debug, PartialEq)]
pub struct InternalPopulation {
    pub persons: Vec<InternalPerson>,
}

impl InternalPerson {
    pub fn new(id: Id<InternalPerson>, plan: InternalPlan) -> Self {
        InternalPerson {
            id,
            plans: vec![plan],
            attributes: None,
        }
    }

    pub fn id(&self) -> &Id<InternalPerson> {
        &self.id
    }

    pub fn curr_act(&self) -> &InternalActivity {
        // if self.curr_plan_elem % 2 != 0 {
        //     panic!("Current element is not an activity");
        // }
        // let act_index = self.curr_plan_elem / 2;
        // self.get_act_at_index(act_index)
        todo!()
    }

    pub fn curr_leg(&self) -> &InternalLeg {
        // if self.curr_plan_elem % 2 != 1 {
        //     panic!("Current element is not a leg.");
        // }
        //
        // let leg_index = (self.curr_plan_elem - 1) / 2;
        // self.plan
        //     .as_ref()
        //     .unwrap()
        //     .legs
        //     .get(leg_index as usize)
        //     .unwrap()
        todo!()
    }

    pub fn next_leg(&self) -> Option<&InternalLeg> {
        // position index: 0      | 1
        // activities:     a0 (0) | a1 (2)
        // legs:           l0 (1) | l1 (3)
        // e.g., if current is a1, next leg is l1 => curr_plan_elem/2
        // e.g., if current is l0, next leg is l1 => (curr_plan_elem + 1)/2

        // let next_leg_index = if self.curr_plan_elem % 2 == 0 {
        //     // current element is an activity
        //     self.curr_plan_elem / 2
        // } else {
        //     // current element is a leg
        //     (self.curr_plan_elem + 1) / 2
        // };
        //
        // self.plan
        //     .as_ref()
        //     .unwrap()
        //     .legs
        //     .get(next_leg_index as usize)
        todo!()
    }

    fn get_act_at_index(&self, index: u32) -> &InternalActivity {
        // self.plan
        //     .as_ref()
        //     .unwrap()
        //     .acts
        //     .get(index as usize)
        //     .unwrap()
        todo!()
    }

    pub fn advance_plan(&mut self) {
        // let next = self.curr_plan_elem + 1;
        // if self.plan.as_ref().unwrap().acts.len() + self.plan.as_ref().unwrap().legs.len()
        //     == next as usize
        // {
        //     panic!(
        //         "Person: Advance plan was called on Person #{}, but no element is remaining.",
        //         self.id
        //     )
        // }
        // self.curr_plan_elem = next;
        todo!()
    }

    pub fn legs(&self) -> &[InternalLeg] {
        // self.plan.as_ref().unwrap().legs.as_slice()
        todo!()
    }

    pub fn acts(&self) -> &[InternalActivity] {
        // self.plan.as_ref().unwrap().acts.as_slice()
        todo!()
    }

    pub fn selected_plan(&self) -> Option<&InternalPlan> {
        self.plans.iter().find(|&plan| plan.selected)
    }
}

impl Default for InternalPlan {
    fn default() -> Self {
        Self {
            selected: true,
            elements: Vec::new(),
        }
    }
}

impl InternalPlan {
    pub fn add_leg(&mut self, leg: InternalLeg) {
        self.elements.push(InternalPlanElement::Leg(leg));
    }

    pub fn add_act(&mut self, activity: InternalActivity) {
        self.elements.push(InternalPlanElement::Activity(activity));
    }

    pub fn legs(&self) -> &Vec<InternalLeg> {
        todo!()
    }

    pub fn acts(&self) -> &Vec<InternalActivity> {
        todo!()
    }
}

impl InternalActivity {
    pub fn new(
        x: f64,
        y: f64,
        act_type: &str,
        link_id: Id<Link>,
        start_time: Option<u32>,
        end_time: Option<u32>,
        max_dur: Option<u32>,
    ) -> Self {
        InternalActivity {
            x,
            y,
            act_type: Id::create(&act_type),
            link_id,
            start_time,
            end_time,
            max_dur,
        }
    }

    pub(crate) fn cmp_end_time(&self, now: u32) -> u32 {
        if let Some(end_time) = self.end_time {
            end_time
        } else if let Some(max_dur) = self.max_dur {
            now + max_dur
        } else {
            // supposed to be an equivalent for OptionalTime.undefined() in the java code
            u32::MAX
        }
    }

    pub fn is_interaction(&self) -> bool {
        self.act_type.external().contains("interaction")
    }
}

impl InternalRoute {
    pub fn as_generic(&self) -> &InternalGenericRoute {
        match self {
            InternalRoute::Generic(g) => g,
            InternalRoute::Network(n) => &n.generic_delegate,
            InternalRoute::Pt(p) => &p.generic_delegate,
        }
    }

    pub fn as_network(&self) -> Option<&InternalNetworkRoute> {
        match self {
            InternalRoute::Network(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_pt(&self) -> Option<&InternalPtRoute> {
        match self {
            InternalRoute::Pt(p) => Some(p),
            _ => None,
        }
    }

    pub fn start_link(&self) -> &Id<Link> {
        &self.as_generic().start_link
    }

    pub fn end_link(&self) -> &Id<Link> {
        &self.as_generic().start_link
    }
}

impl InternalLeg {
    pub fn new(route: InternalRoute, mode: &str, trav_time: u32, dep_time: Option<u32>) -> Self {
        Self {
            route: Some(route),
            mode: Id::create(mode),
            trav_time: Some(trav_time),
            dep_time,
            attributes: None,
        }
    }

    fn parse_trav_time(leg_trav_time: &Option<String>, route_trav_time: &Option<String>) -> u32 {
        if let Some(trav_time) = parse_time_opt(leg_trav_time) {
            trav_time
        } else {
            parse_time_opt(route_trav_time).unwrap_or(0)
        }
    }
}

fn trim_quotes(s: &Value) -> String {
    s.to_string().trim_matches('"').to_string()
}

fn parse_time_opt(value: &Option<String>) -> Option<u32> {
    if let Some(time) = value.as_ref() {
        parse_time(time)
    } else {
        None
    }
}

fn parse_time(value: &str) -> Option<u32> {
    let split: Vec<&str> = value.split(':').collect();
    if split.len() == 3 {
        let hour: u32 = split.first().unwrap().parse().unwrap();
        let minutes: u32 = split.get(1).unwrap().parse().unwrap();
        let seconds: u32 = split.get(2).unwrap().parse().unwrap();

        Some(hour * 3600 + minutes * 60 + seconds)
    } else {
        None
    }
}

impl From<IOPerson> for InternalPerson {
    fn from(io: IOPerson) -> Self {
        InternalPerson {
            id: Id::create(&io.id),
            plans: io.plans.into_iter().map(InternalPlan::from).collect(),
            attributes: io.attributes,
        }
    }
}

impl From<IOPlanElement> for InternalPlanElement {
    fn from(io: IOPlanElement) -> Self {
        match io {
            IOPlanElement::Activity(act) => InternalPlanElement::Activity(InternalActivity {
                act_type: Id::create(&act.r#type),
                link_id: Id::create(&act.link),
                x: act.x,
                y: act.y,
                start_time: act.start_time.and_then(|s| s.parse().ok()),
                end_time: act.end_time.and_then(|s| s.parse().ok()),
                max_dur: act.max_dur.and_then(|s| s.parse().ok()),
            }),
            IOPlanElement::Leg(leg) => InternalPlanElement::Leg(InternalLeg {
                mode: Id::create(&leg.mode),
                dep_time: leg.dep_time.and_then(|s| s.parse().ok()),
                trav_time: leg.trav_time.and_then(|s| s.parse().ok()),
                route: leg.route.map(InternalRoute::from),
                attributes: leg
                    .attributes
                    .and_then(|a| InternalAttributes::from(a).into()),
            }),
        }
    }
}

impl From<IORoute> for InternalRoute {
    fn from(io: IORoute) -> Self {
        todo!()
        // InternalRoute {
        //     r#type: Id::create(&io.r#type),
        //     start_link: Id::create(&io.start_link),
        //     end_link: Id::create(&io.end_link),
        //     trav_time: io.trav_time.and_then(|s| s.parse().ok()),
        //     distance: io.distance,
        //     vehicle: io.vehicle.map(|v| Id::create(&v)),
        //     route: todo!("Convert route links from IORoute to InternalRoute"),
        // }
    }
}

impl From<IOPlan> for InternalPlan {
    fn from(io: IOPlan) -> Self {
        InternalPlan {
            selected: io.selected,
            elements: io
                .elements
                .into_iter()
                .map(InternalPlanElement::from)
                .collect(),
        }
    }
}
