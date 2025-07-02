use crate::simulation::id::Id;
use crate::simulation::io::attributes::IOAttributes;
use crate::simulation::io::proto::general::attribute_value::Type;
use crate::simulation::io::proto::general::AttributeValue;
use crate::simulation::messaging::messages::SimulationAgentState;
use crate::simulation::network::global_network::Link;
use crate::simulation::population::{
    InternalActivity, InternalLeg, InternalPerson, InternalPlanElement, InternalRoute,
};
use crate::simulation::time_queue::{EndTime, Identifiable};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::warn;

pub mod config;
pub mod controller;
pub mod engines;
pub mod id;
pub mod io;
pub mod logging;
pub mod messaging;
pub mod network;
pub mod population;
pub mod profiling;
pub mod pt;
pub mod replanning;
pub mod scenario;
#[allow(clippy::module_inception)]
pub mod simulation;
pub mod time_queue;
pub mod vehicles;

#[derive(Debug, PartialEq, Clone)]
pub struct InternalSimulationAgent {
    logic: InternalSimulationAgentLogic,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalSimulationAgentLogic {
    basic_agent_delegate: InternalPerson,
    curr_plan_element: usize,
    curr_route_element: usize,
}

impl EndTime for InternalSimulationAgent {
    fn end_time(&self, now: u32) -> u32 {
        self.logic.end_time(now)
    }
}

impl Identifiable<InternalPerson> for InternalSimulationAgent {
    fn id(&self) -> &Id<InternalPerson> {
        self.logic.id()
    }
}

impl InternalSimulationAgent {
    pub fn new(person: InternalPerson) -> Self {
        Self {
            logic: InternalSimulationAgentLogic {
                basic_agent_delegate: person,
                curr_plan_element: 0,
                curr_route_element: 0,
            },
        }
    }

    pub fn id(&self) -> &Id<InternalPerson> {
        &self.logic.basic_agent_delegate.id()
    }

    pub fn curr_act(&self) -> &InternalActivity {
        self.logic.curr_act()
    }

    pub fn curr_leg(&self) -> &InternalLeg {
        self.logic.curr_leg()
    }

    pub fn next_leg(&self) -> Option<&InternalLeg> {
        self.logic.next_leg()
    }

    pub fn advance_plan(&mut self) {
        self.logic.advance_plan();
    }

    pub fn wakeup_time(&self, now: u32) -> u32 {
        self.logic.wakeup_time(now)
    }

    pub fn state(&self) -> SimulationAgentState {
        self.logic.state()
    }

    pub fn curr_link_id(&self) -> Option<&Id<Link>> {
        self.logic.curr_link_id()
    }

    pub fn peek_next_link_id(&self) -> Option<&Id<Link>> {
        self.logic.peek_next_link_id()
    }

    pub fn register_moved_to_next_link(&mut self) {
        self.logic.register_moved_to_next_link();
    }

    pub fn register_vehicle_exited(&mut self) {
        self.logic.register_vehicle_exited();
    }

    pub fn route_index_to_last(&mut self) {
        self.logic.route_index_to_last();
    }
}

impl InternalSimulationAgentLogic {
    pub(crate) fn curr_link_id(&self) -> Option<&Id<Link>> {
        if self.state() != SimulationAgentState::LEG {
            return None;
        }

        match self.curr_leg().route.as_ref().unwrap() {
            InternalRoute::Generic(g) => match self.curr_route_element {
                0 => Some(g.start_link()),
                1 => Some(g.end_link()),
                _ => panic!(
                    "A generic route only has two elements. Current plan element {:?}, Current route element {:?}, Current agent {:?}", self.curr_plan_element, self.curr_route_element, self.basic_agent_delegate.id()
                ),
            },
            InternalRoute::Network(n) => n.route_element_at(self.curr_route_element),
            InternalRoute::Pt(p) => match self.curr_route_element {
                0 => Some(p.start_link()),
                1 => Some(p.end_link()),
                _ => panic!(
                    "A generic route only has two elements. Current plan element {:?}, Current route element {:?}, Current agent {:?}", self.curr_plan_element, self.curr_route_element, self.basic_agent_delegate.id()
                ),
            },
        }
    }

    pub fn peek_next_link_id(&self) -> Option<&Id<Link>> {
        let next_i = self.curr_route_element + 1;
        self.curr_leg()
            .route
            .as_ref()
            .unwrap()
            .as_network()
            .unwrap()
            .route_element_at(next_i)
    }

    pub fn id(&self) -> &Id<InternalPerson> {
        self.basic_agent_delegate.id()
    }

    pub fn curr_act(&self) -> &InternalActivity {
        self.basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
            .as_activity()
            .unwrap()
    }

    pub fn curr_leg(&self) -> &InternalLeg {
        self.basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
            .as_leg()
            .unwrap()
    }

    pub fn next_leg(&self) -> Option<&InternalLeg> {
        let add = if self.curr_plan_element % 2 == 0 {
            // If the current plan element is an activity, the next one should be a leg
            1
        } else {
            // If the current plan element is a leg, the next one should be an activity
            2
        };
        self.basic_agent_delegate
            .plan_element_at(self.curr_plan_element + add)
            .as_leg()
    }

    pub fn advance_plan(&mut self) {
        self.curr_plan_element += 1;
        self.curr_route_element = 0;
        assert!(
            self.curr_plan_element < self.basic_agent_delegate.total_elements(),
            "Cannot advance plan of agents {:?} beyond its last element.",
            self.basic_agent_delegate.id()
        );
    }

    pub fn state(&self) -> SimulationAgentState {
        match self.curr_plan_element % 2 {
            0 => SimulationAgentState::ACTIVITY,
            1 => SimulationAgentState::LEG,
            _ => unreachable!(),
        }
    }

    pub fn wakeup_time(&self, now: u32) -> u32 {
        // TODO this might be adapted with rolling horizon logic

        match self
            .basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
        {
            InternalPlanElement::Activity(a) => a.cmp_end_time(now),
            InternalPlanElement::Leg(_) => panic!("Cannot wake up on a leg!"),
        }
    }

    pub fn register_moved_to_next_link(&mut self) {
        self.curr_route_element += 1;
    }

    pub fn register_vehicle_exited(&mut self) {
        self.curr_route_element += 1;
    }

    /// This method advances the pointer to the last element of the route. We need this in case of
    /// teleported legs. Advancing the route pointer to the last element directly ensures that teleporting
    /// the vehicle is independent of whether the leg has a Generic-Teleportation route or a network
    /// route.
    pub fn route_index_to_last(&mut self) {
        let route = self.curr_leg().route.as_ref().unwrap();
        if route.as_network().is_some() {
            let last = route.as_network().unwrap().route().len() - 1;
            self.curr_route_element = last;
        } else {
            self.curr_route_element = 1;
        }
    }
}

impl EndTime for InternalSimulationAgentLogic {
    fn end_time(&self, now: u32) -> u32 {
        match self
            .basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
        {
            InternalPlanElement::Activity(a) => a.cmp_end_time(now),
            InternalPlanElement::Leg(l) => l.trav_time.unwrap() + now,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Clone)]
pub struct InternalAttributes {
    // we are using serde_json::Value to allow for flexible attribute types and serializability
    attributes: HashMap<String, serde_json::Value>,
}

impl InternalAttributes {
    pub fn insert<T: Serialize>(&mut self, key: impl Into<String>, value: T) {
        self.attributes.insert(key.into(), json!(value));
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.attributes
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, Value> {
        self.attributes.iter()
    }

    pub fn as_cloned_map(&self) -> HashMap<String, AttributeValue> {
        let mut attributes = HashMap::new();
        for (key, value) in self.iter() {
            let insert = match value {
                Value::Bool(b) => AttributeValue::new_bool(*b),
                Value::Number(n) => {
                    if n.is_i64() {
                        AttributeValue::new_int(n.as_i64().unwrap())
                    } else if n.is_f64() {
                        AttributeValue::new_double(n.as_f64().unwrap())
                    } else {
                        warn!("Unsupported number type for key '{}': {:?}", key, n);
                        continue;
                    }
                }
                Value::String(s) => AttributeValue::new_string(s.clone()),
                _ => {
                    warn!("Unsupported attribute type for key '{}': {:?}", key, value);
                    continue;
                }
            };
            attributes.insert(key.clone(), insert);
        }
        attributes
    }
}

impl From<IOAttributes> for InternalAttributes {
    fn from(attrs: IOAttributes) -> Self {
        let mut res = InternalAttributes::default();
        for attr in attrs.attributes {
            match attr.class.as_str() {
                "java.lang.Integer" => res.insert(attr.name, attr.value.parse::<i32>().unwrap()),
                "java.lang.Long" => res.insert(attr.name, attr.value.parse::<i64>().unwrap()),
                "java.lang.Double" => res.insert(attr.name, attr.value.parse::<f64>().unwrap()),
                "java.lang.String" => res.insert(attr.name, attr.value),
                "java.lang.Boolean" => res.insert(attr.name, attr.value.parse::<bool>().unwrap()),
                _ => warn!("Unknown attribute class {:?}. Skipping...", attr.class),
            };
        }
        res
    }
}

impl From<HashMap<String, AttributeValue>> for InternalAttributes {
    fn from(map: HashMap<String, AttributeValue>) -> Self {
        let mut res = InternalAttributes::default();
        for (key, value) in map {
            match value.r#type.as_ref().unwrap() {
                Type::IntValue(i) => {
                    res.insert(key, i);
                }
                Type::StringValue(s) => {
                    res.insert(key, s);
                }
                Type::DoubleValue(d) => {
                    res.insert(key, d);
                }
                Type::BoolValue(b) => {
                    res.insert(key, b);
                }
            };
        }
        res
    }
}
