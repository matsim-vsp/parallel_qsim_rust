use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Attr {
    pub name: String,
    pub class: String,
    #[serde(rename = "$value")]
    pub value: String,
}

impl Attr {
    pub fn new(name: String, value: String) -> Self {
        Attr {
            name,
            class: "".to_string(),
            value,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Default)]
pub struct Attrs {
    #[serde(rename = "attribute", default)]
    pub attributes: Vec<Attr>,
}

impl Attrs {
    pub fn find_or_else<'a, F>(&self, name: &str, f: F) -> &str
    where
        F: FnOnce() -> &'a str,
    {
        let opt_attr = self.attributes.iter().find(|&attr| attr.name.eq(name));
        let value = if let Some(&attr) = opt_attr {
            attr.value.as_str()
        } else {
            f()
        };

        value
    }
}
