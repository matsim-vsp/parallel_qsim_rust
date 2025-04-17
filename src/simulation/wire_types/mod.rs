// Include the `messages` module, which is generated from messages.proto
pub mod events {
    include!(concat!(env!("OUT_DIR"), "/events.rs"));
}
pub mod ids {
    include!(concat!(env!("OUT_DIR"), "/ids.rs"));
}
pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

pub mod network {
    include!(concat!(env!("OUT_DIR"), "/network.rs"));
}
pub mod population {
    include!(concat!(env!("OUT_DIR"), "/population.rs"));
}
pub mod vehicles {
    include!(concat!(env!("OUT_DIR"), "/vehicles.rs"));
}

pub mod general {
    include!(concat!(env!("OUT_DIR"), "/general.rs"));
}

#[allow(clippy::module_inception)]
pub mod wire_types {
    use crate::simulation::io::attributes::Attr;
    use crate::simulation::wire_types::general::attribute_value::Type;
    use crate::simulation::wire_types::general::AttributeValue;

    impl AttributeValue {
        fn new_int(value: i64) -> Self {
            AttributeValue {
                r#type: Some(Type::IntValue(value)),
            }
        }

        fn new_string(value: String) -> Self {
            AttributeValue {
                r#type: Some(Type::StringValue(value)),
            }
        }

        fn new_double(value: f64) -> Self {
            AttributeValue {
                r#type: Some(Type::DoubleValue(value)),
            }
        }

        fn new_bool(value: bool) -> Self {
            AttributeValue {
                r#type: Some(Type::BoolValue(value)),
            }
        }

        pub fn as_int(&self) -> i64 {
            match self.r#type.as_ref().unwrap() {
                Type::IntValue(value) => *value,
                _ => panic!("Expected int, got {:?}", self),
            }
        }

        pub fn as_string(&self) -> String {
            match self.r#type.as_ref().unwrap() {
                Type::StringValue(value) => value.clone(),
                _ => panic!("Expected string, got {:?}", self),
            }
        }

        pub fn as_double(&self) -> f64 {
            match self.r#type.as_ref().unwrap() {
                Type::DoubleValue(value) => *value,
                _ => panic!("Expected double, got {:?}", self),
            }
        }

        pub fn as_bool(&self) -> bool {
            match self.r#type.as_ref().unwrap() {
                Type::BoolValue(value) => *value,
                _ => panic!("Expected bool, got {:?}", self),
            }
        }

        pub fn from_io_attr(attr: Attr) -> AttributeValue {
            match attr.class.as_str() {
                "java.lang.String" => AttributeValue::new_string(attr.value),
                "java.lang.Double" => AttributeValue::new_double(attr.value.parse().unwrap()),
                "java.lang.Integer" => AttributeValue::new_int(attr.value.parse().unwrap()),
                "java.lang.Boolean" => AttributeValue::new_bool(attr.value.parse().unwrap()),
                _ => panic!("Unsupported attribute class: {}", attr.class),
            }
        }
    }
}
