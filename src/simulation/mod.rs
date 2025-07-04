use crate::generated::general::attribute_value::Type;
use crate::generated::general::AttributeValue;
use crate::simulation::controller::local_controller::ComputationalEnvironment;
use crate::simulation::id::Id;
use crate::simulation::population::{
    InternalActivity, InternalLeg, InternalPerson, InternalPlanElement, InternalRoute,
};
use crate::simulation::time_queue::{EndTime, Identifiable};
use io::xml::attributes::IOAttributes;
use network::Link;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt::Debug;
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

pub trait SimulationAgentLogic:
    EndTime + Identifiable<InternalPerson> + EnvironmentalEventObserver + Send
{
    fn curr_act(&self) -> &InternalActivity;
    fn curr_leg(&self) -> &InternalLeg;
    fn next_leg(&self) -> Option<&InternalLeg>;
    fn advance_plan(&mut self);
    fn wakeup_time(&self, now: u32) -> u32;
    fn state(&self) -> SimulationAgentState;
    fn curr_link_id(&self) -> Option<&Id<Link>>;
    fn peek_next_link_id(&self) -> Option<&Id<Link>>;
}

pub trait EnvironmentalEventObserver {
    fn notify_event(&mut self, event: AgentEvent, now: u32);
}

#[non_exhaustive]
#[derive(Clone)]
pub enum AgentEvent {
    ActivityStarted { comp_env: ComputationalEnvironment },
    Wakeup { comp_env: ComputationalEnvironment },
    ActivityFinished { comp_env: ComputationalEnvironment },
    TeleportationStarted { comp_env: ComputationalEnvironment },
    TeleportationFinished { comp_env: ComputationalEnvironment },
    NetworkLegStarted { comp_env: ComputationalEnvironment },
    MovedToNextLink { comp_env: ComputationalEnvironment },
    NetworkLegFinished { comp_env: ComputationalEnvironment },
}

#[derive(Debug)]
pub struct SimulationAgent {
    logic: Box<dyn SimulationAgentLogic>,
}

impl Debug for dyn SimulationAgentLogic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Simulation Agent Logic for agent with id {}",
            self.id().external()
        )
    }
}

impl PartialEq for SimulationAgent {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct PlanBasedSimulationLogic {
    basic_agent_delegate: InternalPerson,
    curr_plan_element: usize,
    curr_route_element: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SimulationAgentState {
    LEG,
    ACTIVITY,
    STUCK,
}

impl SimulationAgent {
    pub fn new(person: InternalPerson) -> Self {
        Self {
            logic: Box::new(PlanBasedSimulationLogic {
                basic_agent_delegate: person,
                curr_plan_element: 0,
                curr_route_element: 0,
            }),
        }
    }
}

impl EndTime for SimulationAgent {
    fn end_time(&self, now: u32) -> u32 {
        self.logic.end_time(now)
    }
}

impl Identifiable<InternalPerson> for SimulationAgent {
    fn id(&self) -> &Id<InternalPerson> {
        self.logic.id()
    }
}

impl EnvironmentalEventObserver for SimulationAgent {
    fn notify_event(&mut self, event: AgentEvent, now: u32) {
        self.logic.notify_event(event, now)
    }
}

impl SimulationAgentLogic for SimulationAgent {
    fn curr_act(&self) -> &InternalActivity {
        self.logic.curr_act()
    }
    fn curr_leg(&self) -> &InternalLeg {
        self.logic.curr_leg()
    }
    fn next_leg(&self) -> Option<&InternalLeg> {
        self.logic.next_leg()
    }
    fn advance_plan(&mut self) {
        self.logic.advance_plan();
    }
    fn wakeup_time(&self, now: u32) -> u32 {
        self.logic.wakeup_time(now)
    }
    fn state(&self) -> SimulationAgentState {
        self.logic.state()
    }
    fn curr_link_id(&self) -> Option<&Id<Link>> {
        self.logic.curr_link_id()
    }
    fn peek_next_link_id(&self) -> Option<&Id<Link>> {
        self.logic.peek_next_link_id()
    }
}

impl Identifiable<InternalPerson> for PlanBasedSimulationLogic {
    fn id(&self) -> &Id<InternalPerson> {
        self.basic_agent_delegate.id()
    }
}

impl EnvironmentalEventObserver for PlanBasedSimulationLogic {
    fn notify_event(&mut self, event: AgentEvent, _now: u32) {
        match event {
            AgentEvent::TeleportationStarted { .. } => {
                self.set_curr_route_element_to_last();
            }
            AgentEvent::MovedToNextLink { .. } => {
                self.curr_route_element += 1;
            }
            _ => {}
        }
    }
}

impl PlanBasedSimulationLogic {
    /// This method advances the pointer to the last element of the route. We need this in case of
    /// teleported legs. Advancing the route pointer to the last element directly ensures that teleporting
    /// the vehicle is independent of whether the leg has a Generic-Teleportation route or a network
    /// route.
    fn set_curr_route_element_to_last(&mut self) {
        let route = self.curr_leg().route.as_ref().unwrap();
        if route.as_network().is_some() {
            let last = route.as_network().unwrap().route().len() - 1;
            self.curr_route_element = last;
        } else {
            self.curr_route_element = 1;
        }
    }
}

impl SimulationAgentLogic for PlanBasedSimulationLogic {
    fn curr_act(&self) -> &InternalActivity {
        self.basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
            .as_activity()
            .unwrap()
    }

    fn curr_leg(&self) -> &InternalLeg {
        self.basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
            .as_leg()
            .unwrap()
    }

    fn next_leg(&self) -> Option<&InternalLeg> {
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

    fn advance_plan(&mut self) {
        self.curr_plan_element += 1;
        self.curr_route_element = 0;
        assert!(
            self.curr_plan_element < self.basic_agent_delegate.total_elements(),
            "Cannot advance plan of agents {:?} beyond its last element.",
            self.basic_agent_delegate.id()
        );
    }

    fn wakeup_time(&self, now: u32) -> u32 {
        // TODO this might be adapted with rolling horizon logic

        match self
            .basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
        {
            InternalPlanElement::Activity(a) => a.cmp_end_time(now),
            InternalPlanElement::Leg(_) => panic!("Cannot wake up on a leg!"),
        }
    }

    fn state(&self) -> SimulationAgentState {
        match self.curr_plan_element % 2 {
            0 => SimulationAgentState::ACTIVITY,
            1 => SimulationAgentState::LEG,
            _ => unreachable!(),
        }
    }

    fn curr_link_id(&self) -> Option<&Id<Link>> {
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

    fn peek_next_link_id(&self) -> Option<&Id<Link>> {
        let next_i = self.curr_route_element + 1;
        self.curr_leg()
            .route
            .as_ref()
            .unwrap()
            .as_network()
            .unwrap()
            .route_element_at(next_i)
    }
}

impl EndTime for PlanBasedSimulationLogic {
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
