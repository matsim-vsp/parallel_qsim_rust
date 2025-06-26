use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::messaging::messages::SimulationAgentState;
use crate::simulation::population::{InternalActivity, InternalLeg, InternalPerson};
use crate::simulation::time_queue::{EndTime, Identifiable};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
pub mod wire_types;

#[derive(Debug, PartialEq, Clone)]
pub struct InternalSimulationAgent {
    pub(crate) logic: InternalSimulationAgentLogic,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalSimulationAgentLogic {
    pub(crate) basic_agent_delegate: InternalPerson,
}

impl EndTime for InternalSimulationAgent {
    fn end_time(&self, now: u32) -> u32 {
        self.logic.end_time(now)
    }
}

impl Identifiable for InternalSimulationAgent {
    fn id(&self) -> u64 {
        self.logic.id()
    }
}

impl InternalSimulationAgent {
    pub fn new(person: InternalPerson) -> Self {
        Self {
            logic: InternalSimulationAgentLogic {
                basic_agent_delegate: person,
            },
        }
    }

    pub fn id(&self) -> &Id<InternalPerson> {
        &self.logic.basic_agent_delegate.id
    }

    pub fn curr_act(&self) -> &InternalActivity {
        todo!()
    }

    pub fn curr_leg(&self) -> &InternalLeg {
        todo!()
    }

    pub fn next_leg(&self) -> Option<&InternalLeg> {
        todo!()
    }

    pub fn advance_plan(&mut self) {
        todo!()
    }

    pub fn state(&self) -> SimulationAgentState {
        todo!()
    }

    pub fn wakeup_time(&self, now: u32) -> u32 {
        todo!()
    }
}

impl InternalSimulationAgentLogic {
    pub fn end_time(&self, now: u32) -> u32 {
        todo!()
    }

    pub fn id(&self) -> u64 {
        todo!()
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

    fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.attributes
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

impl From<Attrs> for InternalAttributes {
    fn from(attrs: Attrs) -> Self {
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
