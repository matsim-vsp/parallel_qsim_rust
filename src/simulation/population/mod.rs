use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::network::global_network::Link;
use crate::simulation::population::io::{IOPerson, IOPlan, IOPlanElement, IORoute};
use crate::simulation::wire_types::messages::Vehicle;
use crate::simulation::InternalAttributes;

pub mod agent_source;
pub mod io;
pub mod population_data;

#[derive(Debug, PartialEq)]
pub struct InternalActivity {
    pub act_type: Id<String>,
    pub link_id: Id<Link>,
    pub x: f64,
    pub y: f64,
    pub start_time: Option<u32>,
    pub end_time: Option<u32>,
    pub max_dur: Option<u32>,
}

#[derive(Debug, PartialEq)]
pub struct InternalLeg {
    pub mode: Id<String>,
    pub dep_time: Option<u32>,
    pub trav_time: Option<u32>,
    pub route: Option<InternalRoute>,
    pub attributes: Option<InternalAttributes>,
}

#[derive(Debug, PartialEq)]
pub struct InternalRoute {
    pub r#type: Id<String>,
    pub start_link: Id<Link>,
    pub end_link: Id<Link>,
    pub trav_time: Option<u32>,
    pub distance: f64,
    pub vehicle: Option<Id<Vehicle>>,
    pub route: Option<Id<Link>>,
}

#[derive(Debug, PartialEq)]
pub enum InternalPlanElement {
    Activity(InternalActivity),
    Leg(InternalLeg),
}

#[derive(Debug, PartialEq)]
pub struct InternalPlan {
    pub selected: bool,
    pub elements: Vec<InternalPlanElement>,
}

#[derive(Debug, PartialEq)]
pub struct InternalPerson {
    pub id: Id<InternalPerson>,
    pub plans: Vec<InternalPlan>,
    pub attributes: Option<Attrs>,
}

#[derive(Debug, PartialEq)]
pub struct InternalPopulation {
    pub persons: Vec<InternalPerson>,
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
        InternalRoute {
            r#type: Id::create(&io.r#type),
            start_link: Id::create(&io.start_link),
            end_link: Id::create(&io.end_link),
            trav_time: io.trav_time.and_then(|s| s.parse().ok()),
            distance: io.distance,
            vehicle: io.vehicle.map(|v| Id::create(&v)),
            route: io.route.map(|r| Id::create(&r)),
        }
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
