use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "vehicles")]
pub struct IODrtVehicles {
    #[serde(rename = "vehicle")]
    pub vehicles: Vec<IODrtVehicle>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IODrtVehicle {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@start_link")]
    pub start_link: String,
    #[serde(rename = "@t_0")]
    pub t_0: u32,
    #[serde(rename = "@t_1")]
    pub t_1: u32,
    #[serde(rename = "@capacity")]
    pub capacity: u32,
}

#[cfg(test)]
mod tests {
    use crate::dvrp::io::IODrtVehicles;

    #[test]
    fn test_read() {
        let s = r#"
        <vehicles>
	        <vehicle id="taxi_one_A" start_link="215" t_0="0" t_1="8000" capacity="2"/>
        </vehicles>
        "#;
        let mut d = quick_xml::de::Deserializer::from_str(s);
        let v: IODrtVehicles = serde_path_to_error::deserialize(&mut d).unwrap();
        assert_eq!(v.vehicles.len(), 1);
        assert_eq!(v.vehicles[0].id, "taxi_one_A");
        assert_eq!(v.vehicles[0].start_link, "215");
        assert_eq!(v.vehicles[0].t_0, 0);
        assert_eq!(v.vehicles[0].t_1, 8000);
        assert_eq!(v.vehicles[0].capacity, 2);
    }
}
