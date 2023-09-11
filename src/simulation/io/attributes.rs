use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Attr {
    pub name: String,
    pub class: String,
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Attrs {
    #[serde(rename = "attribute", default)]
    pub attributes: Vec<Attr>,
}
