use crate::generated::general::attribute_value::Type;
use crate::generated::general::AttributeValue;
use io::xml::attributes::IOAttributes;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt::Debug;
use tracing::warn;

pub mod agents;
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

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Clone)]
pub struct InternalAttributes {
    // we are using serde_json::Value to allow for flexible attribute types and serializability
    attributes: HashMap<String, Value>,
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

    pub fn add(&mut self, key: impl Into<String>, value: impl Serialize) {
        self.attributes
            .insert(key.into(), serde_json::to_value(value).unwrap());
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
                _ => {} //warn!("Unknown attribute class {:?}. Skipping...", attr.class),
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
