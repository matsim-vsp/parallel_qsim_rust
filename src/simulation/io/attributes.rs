use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Attr {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@class")]
    pub class: String,
    #[serde(rename = "$value", default)]
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
    #[allow(clippy::needless_lifetimes)] // lifetimes are in fact needed here i think
    pub fn find_or_else<'a, F>(&'a self, name: &str, f: F) -> &'a str
    where
        F: FnOnce() -> &'a str,
    {
        let opt_attr = self.attributes.iter().find(|&attr| attr.name.eq(name));
        if let Some(&attr) = opt_attr.as_ref() {
            attr.value.as_str()
        } else {
            f()
        }
    }

    pub fn find_or_else_opt<'a, F>(attrs_opt: &'a Option<Attrs>, name: &str, f: F) -> &'a str
    where
        F: FnOnce() -> &'a str,
    {
        if let Some(attrs) = attrs_opt.as_ref() {
            attrs.find_or_else(name, f)
        } else {
            f()
        }
    }
}
