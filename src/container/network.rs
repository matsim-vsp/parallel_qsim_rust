use crate::container::xml_reader;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub struct IONode {
    pub id: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct IOLink {
    pub id: String,
    pub from: String,
    pub to: String,
    pub length: f32,
    pub capacity: f32,
    pub freespeed: f32,
    pub permlanes: f32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Nodes {
    #[serde(rename = "node", default)]
    nodes: Vec<IONode>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Links {
    #[serde(rename = "link", default)]
    links: Vec<IOLink>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "network")]
pub struct IONetwork {
    name: Option<String>,
    nodes: Nodes,
    links: Links,
}

impl IONetwork {
    pub fn nodes(&self) -> &Vec<IONode> {
        &self.nodes.nodes
    }

    pub fn links(&self) -> &Vec<IOLink> {
        &self.links.links
    }

    pub fn from_file(file_path: &str) -> IONetwork {
        xml_reader::read(file_path)
    }
}

#[cfg(test)]
mod tests {
    use quick_xml::de::from_str;
    use std::error::Error;

    use crate::container::network::IONetwork;

    #[test]
    fn read_simple_network() -> Result<(), Box<dyn Error>> {
        let xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
                <!DOCTYPE network SYSTEM \"http://www.matsim.org/files/dtd/network_v1.dtd\">
                <network name=\"test network\">
                    <nodes>
                        <node id=\"1\" x=\"-20000\" y=\"0\"/>
                    </nodes>
                    <links>
                        <link id=\"23\" from=\"15\" to=\"1\" length=\"10000.00\" capacity=\"36000\" freespeed=\"27.78\" permlanes=\"1\"  />
                    </links>
                </network>
            ";

        let result: IONetwork = from_str(xml)?;

        // test overall structure of network
        assert_eq!("test network", result.name.as_ref().unwrap());
        assert_eq!(1, result.nodes().len());
        assert_eq!(1, result.links().len());

        // test node structure
        let node = result.nodes().first().unwrap();
        assert_eq!("1", node.id);
        assert_eq!(-20000., node.x);
        assert_eq!(0.0, node.y);

        // test the link structure
        let link = result.links().first().unwrap();
        assert_eq!("23", link.id);
        assert_eq!("15", link.from);
        assert_eq!("1", link.to);
        assert_eq!(10000.0, link.length);
        assert_eq!(36000.0, link.capacity);
        assert_eq!(27.78, link.freespeed);
        assert_eq!(1.0, link.permlanes);

        Ok(())
    }

    #[test]
    fn read_example_file() {
        let file_path = "./assets/equil-network.xml";
        let network: IONetwork = IONetwork::from_file(file_path);

        // only test some metadata here
        assert_eq!("equil test network", network.name.as_ref().unwrap());
        assert_eq!(15, network.nodes().len());
        assert_eq!(23, network.links().len());
    }

    #[test]
    fn read_example_file_gzipped() {
        let network: IONetwork = IONetwork::from_file("./assets/andorra-network.xml.gz");

        assert_eq!(2259, network.nodes().len());
        assert_eq!(4288, network.links().len());
    }
}
