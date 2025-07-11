use crate::simulation::io::xml;
use crate::simulation::io::xml::attributes::{IOAttribute, IOAttributes};
use crate::simulation::network::Network;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::Path;
use tracing::info;

pub(crate) fn load_from_xml(path: &Path) -> Network {
    let io_net = IONetwork::from_file(path.to_str().unwrap());
    Network::from(io_net)
}

pub(crate) fn write_to_xml(network: &Network, path: &Path) {
    let mut result = IONetwork::new(None);

    for node in network.nodes() {
        let attributes = IOAttributes {
            attributes: vec![
                IOAttribute {
                    name: "partition".to_string(),
                    value: node.partition.to_string(),
                    class: "java.lang.Integer".to_string(),
                },
                IOAttribute {
                    name: "cmp_weight".to_string(),
                    class: "java.lang.Integer".to_string(),
                    value: node.cmp_weight.to_string(),
                },
            ],
        };
        let io_node = IONode {
            id: node.id.external().to_string(),
            x: node.x,
            y: node.y,
            attributes: Some(attributes),
        };
        result.nodes_mut().push(io_node);
    }

    for link in network.links() {
        let modes = link
            .modes
            .iter()
            .map(|m| m.external().to_string())
            .reduce(|modes, mode| format!("{modes},{mode}"))
            .unwrap();
        let attributes = IOAttributes {
            attributes: vec![IOAttribute {
                name: String::from("partition"),
                value: link.partition.to_string(),
                class: String::from("java.lang.Integer"),
            }],
        };

        let io_link = IOLink {
            id: link.id.external().to_string(),
            from: link.from.external().to_string(),
            to: link.to.external().to_string(),
            length: link.length,
            capacity: link.capacity,
            freespeed: link.freespeed,
            permlanes: link.permlanes,
            modes,
            attributes: Some(attributes),
        };
        result.links.effective_cell_size = Some(network.effective_cell_size());
        result.links_mut().push(io_link);
    }

    result.to_file(path);
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IONode {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@x")]
    pub x: f64,
    #[serde(rename = "@y")]
    pub y: f64,
    #[serde(rename = "attributes", skip_serializing_if = "Option::is_none")]
    pub attributes: Option<IOAttributes>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Default, Clone)]
pub struct IOLink {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@from")]
    pub from: String,
    #[serde(rename = "@to")]
    pub to: String,
    #[serde(rename = "@length")]
    pub length: f64,
    #[serde(rename = "@capacity")]
    pub capacity: f32,
    #[serde(rename = "@freespeed")]
    pub freespeed: f32,
    #[serde(rename = "@permlanes")]
    pub permlanes: f32,
    #[serde(default, rename = "@modes")]
    pub modes: String,
    #[serde(rename = "attributes", skip_serializing_if = "Option::is_none")]
    pub attributes: Option<IOAttributes>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Nodes {
    #[serde(rename = "node", default)]
    pub nodes: Vec<IONode>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Links {
    #[serde(rename = "link", default)]
    pub links: Vec<IOLink>,
    #[serde(rename = "@effectivecellsize")]
    pub effective_cell_size: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "network")]
pub struct IONetwork {
    #[serde(rename = "@name")]
    pub name: Option<String>,
    nodes: Nodes,
    links: Links,
}

impl IONetwork {
    pub fn new(name: Option<String>) -> IONetwork {
        IONetwork {
            links: Links {
                links: Vec::new(),
                effective_cell_size: Some(7.5),
            },
            nodes: Nodes { nodes: Vec::new() },
            name,
        }
    }
    pub fn nodes(&self) -> &Vec<IONode> {
        &self.nodes.nodes
    }

    pub fn nodes_mut(&mut self) -> &mut Vec<IONode> {
        &mut self.nodes.nodes
    }

    pub fn links(&self) -> &Vec<IOLink> {
        &self.links.links
    }

    pub fn links_mut(&mut self) -> &mut Vec<IOLink> {
        &mut self.links.links
    }

    pub fn effective_cell_size(&self) -> f32 {
        self.links.effective_cell_size.unwrap_or(7.5)
    }

    pub fn from_file(file_path: &str) -> IONetwork {
        let network: IONetwork = xml::read_from_file(file_path);
        info!(
            "IONetwork:: Finished reading network. It contains {} nodes and {} links.",
            network.nodes().len(),
            network.links().len()
        );

        network
    }

    pub fn to_file(&self, path: &Path) {
        xml::write_to_file(
            self,
            path,
            "<!DOCTYPE network SYSTEM \"http://www.matsim.org/files/dtd/network_v2.dtd\">",
        );
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::path::PathBuf;

    use quick_xml::de::from_str;

    use crate::simulation::io::xml::network::IONetwork;

    static OUTPUT_FOLDER: &str = "./test_output/io/network/";

    fn get_output_folder(name: &str) -> PathBuf {
        let path = format!("{OUTPUT_FOLDER}{name}/");
        PathBuf::from(&path)
    }

    fn clear_output_folder(name: &str) {
        let folder_path = get_output_folder(name);

        if let Ok(iter) = fs::read_dir(folder_path) {
            for entry in iter {
                fs::remove_file(entry.unwrap().path()).unwrap();
            }
        }
    }

    #[test]
    fn write_and_read_simple_network() {
        // set up
        let test_name = "write_and_read_simple_network";
        clear_output_folder(test_name);

        let xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
                <!DOCTYPE network SYSTEM \"http://www.matsim.org/files/dtd/network_v1.dtd\">
                <network name=\"test network\">
                    <nodes>
                        <node id=\"1\" x=\"-20000\" y=\"0\"/>
                    </nodes>
                    <links effectivecellsize=\"385.3\">
                        <link id=\"23\" from=\"15\" to=\"1\" length=\"10000.00\" capacity=\"36000\" freespeed=\"27.78\" permlanes=\"1\" modes=\"car,bike\"  />
                    </links>
                </network>
            ";

        let network: IONetwork = from_str(xml).unwrap();
        let file_path = get_output_folder(test_name).join("network.xml.gz");
        network.to_file(&file_path);

        let result = IONetwork::from_file(file_path.to_str().unwrap());
        assert_eq!(network, result);
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
                    <links effectivecellsize=\"42.0\">
                        <link id=\"23\" from=\"15\" to=\"1\" length=\"10000.00\" capacity=\"36000\" freespeed=\"27.78\" permlanes=\"1\" modes=\"car, bike\" />
                    </links>
                </network>
            ";

        let result: IONetwork = from_str(xml)?;

        // test overall structure of network
        assert_eq!("test network", result.name.as_ref().unwrap());
        assert_eq!(1, result.nodes().len());
        assert_eq!(1, result.links().len());
        assert_eq!(42., result.effective_cell_size());

        // test node structure
        let node = result.nodes().first().unwrap();
        assert_eq!("1", node.id);
        assert_eq!(-20000., node.x);
        assert_eq!(0.0, node.y);

        // test the link structure
        let link = result.links().first().unwrap();
        let modes: Vec<_> = link.modes.split(',').map(|s| s.trim()).collect();
        assert_eq!("23", link.id);
        assert_eq!("15", link.from);
        assert_eq!("1", link.to);
        assert_eq!(10000.0, link.length);
        assert_eq!(36000.0, link.capacity);
        assert_eq!(27.78, link.freespeed);
        assert_eq!(1.0, link.permlanes);
        assert_eq!(vec!["car", "bike"], modes);

        Ok(())
    }

    #[test]
    fn read_simple_network() {
        let file_path = "./assets/io_network_tests/simple-network.xml";
        let network: IONetwork = IONetwork::from_file(file_path);

        assert_eq!("simple network", network.name.as_ref().unwrap());
        assert_eq!(2, network.nodes().len());
        assert_eq!(2, network.links().len());

        for node in network.nodes() {
            match &node.attributes {
                None => {
                    assert_eq!("node-without-attr", node.id);
                }
                Some(attrs) => {
                    assert_eq!(1, attrs.attributes.len());
                    let attr = attrs.attributes.first().unwrap();
                    assert_eq!("test", attr.name);
                    assert_eq!("value", attr.value);
                }
            }
        }

        for link in network.links() {
            match &link.attributes {
                None => {
                    assert_eq!("link-without-attr", link.id);
                }
                Some(attrs) => {
                    assert_eq!("link-with-attr", link.id);
                    assert_eq!(1, attrs.attributes.len());
                    let attr = attrs.attributes.first().unwrap();
                    assert_eq!("test", attr.name);
                    assert_eq!("value", attr.value);
                }
            }
        }
    }

    #[test]
    fn read_example_file() {
        let file_path = "./assets/equil/equil-network.xml";
        let network: IONetwork = IONetwork::from_file(file_path);

        // only test some metadata here
        assert_eq!("equil test network", network.name.as_ref().unwrap());
        assert_eq!(15, network.nodes().len());
        assert_eq!(23, network.links().len());
        // this network doesn't have an effectivecellsize set so test, whether the default works
        assert_eq!(7.5, network.effective_cell_size());
    }

    #[test]
    fn read_example_file_gzipped() {
        let network: IONetwork = IONetwork::from_file("./assets/andorra-network.xml.gz");

        assert_eq!(2259, network.nodes().len());
        assert_eq!(4288, network.links().len());

        assert_eq!(None, network.nodes.nodes.first().unwrap().attributes);
    }
}
