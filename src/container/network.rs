use crate::container::matsim_id::MatsimId;
use crate::container::xml_reader;
use flate2::read::GzEncoder;
use flate2::Compression;
use quick_xml::events::attributes::Attributes;
use quick_xml::se::{to_string, to_writer};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufWriter;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Attr {
    name: String,
    #[serde(rename = "$value")]
    value: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Attrs {
    #[serde(rename = "attribute", default)]
    attributes: Vec<Attr>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct IONode {
    pub id: String,
    pub x: f32,
    pub y: f32,
}

impl MatsimId for IONode {
    fn id(&self) -> &str {
        self.id.as_str()
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct IOLink {
    pub id: String,
    pub from: String,
    pub to: String,
    pub length: f32,
    pub capacity: f32,
    pub freespeed: f32,
    pub permlanes: f32,
    pub attributes: Option<Attrs>,
}

impl MatsimId for IOLink {
    fn id(&self) -> &str {
        self.id.as_str()
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Nodes {
    #[serde(rename = "node", default)]
    nodes: Vec<IONode>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Links {
    #[serde(rename = "link", default)]
    links: Vec<IOLink>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
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

    pub fn to_file(&self, file_path: &str) {
        let file = File::create(file_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::fast());
        let writer = BufWriter::new(encoder);
        to_writer(writer, self).unwrap();

        println!("done");
    }
}

#[cfg(test)]
mod tests {
    use quick_xml::de::from_str;
    use std::error::Error;

    use crate::container::network::IONetwork;

    #[test]
    fn write_simple_network() {
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

        let result: IONetwork = from_str(xml).unwrap();
        result.to_file("./network.xml.gz");
    }

    #[test]
    fn parse_simple_network() -> Result<(), Box<dyn Error>> {
        let xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
                <!DOCTYPE network SYSTEM \"http://www.matsim.org/files/dtd/network_v1.dtd\">
                <network name=\"test network\">
                    <nodes>
                        <node id=\"1\" x=\"-20000\" y=\"0\">
                            <attributes>
                                
                            </attributes>
                        </node>
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
    fn read_simple_network() {
        let file_path = "./assets/io_network_tests/simple-network.xml";
        let network: IONetwork = IONetwork::from_file(file_path);

        assert_eq!("simple network", network.name.as_ref().unwrap());
        assert_eq!(2, network.nodes().len());
        assert_eq!(2, network.links().len());

        println!("{network:#?}");
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
