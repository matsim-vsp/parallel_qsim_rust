use std::fmt::Debug;
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use flate2::Compression;
use quick_xml::se::to_writer;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::simulation::io::attributes::Attrs;
use crate::simulation::io::matsim_id::MatsimId;
use crate::simulation::io::xml_reader;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct IONode {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub attributes: Option<Attrs>,
}

impl MatsimId for IONode {
    fn id(&self) -> &str {
        self.id.as_str()
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Default, Clone)]
pub struct IOLink {
    pub id: String,
    pub from: String,
    pub to: String,
    pub length: f32,
    pub capacity: f32,
    pub freespeed: f32,
    pub permlanes: f32,
    #[serde(default)]
    pub modes: String,
    pub attributes: Option<Attrs>,
}

impl MatsimId for IOLink {
    fn id(&self) -> &str {
        self.id.as_str()
    }
}

impl IOLink {
    pub fn modes(&self) -> Vec<String> {
        if self.modes.eq("") {
            return Vec::new();
        };

        self.modes
            .replace(' ', "")
            .split(',')
            .map(String::from)
            .collect()
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Nodes {
    #[serde(rename = "node", default)]
    pub nodes: Vec<IONode>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Links {
    #[serde(rename = "link", default)]
    pub links: Vec<IOLink>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "network")]
pub struct IONetwork {
    pub name: Option<String>,
    pub nodes: Nodes,
    pub links: Links,
}

impl IONetwork {
    pub fn new(name: Option<String>) -> IONetwork {
        IONetwork {
            links: Links { links: Vec::new() },
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

    pub fn from_file(file_path: &str) -> IONetwork {
        let network: IONetwork = xml_reader::read(file_path);
        info!(
            "IONetwork:: Finished reading network. It contains {} nodes and {} links.",
            network.nodes().len(),
            network.links().len()
        );

        network
    }

    pub fn to_file(&self, path: &Path) {
        // Create the file and all necessary directories
        // this doesn't cover some edge cases, but this will do for now
        //let path = Path::new(file_path);
        let prefix = path.parent().unwrap();
        fs::create_dir_all(prefix).unwrap();
        let file = File::create(path).unwrap();

        // start writing gz stream to the file. This will eventually move into a separate file, once
        // we want to write other stuff as well.
        let encoder = flate2::write::GzEncoder::new(file, Compression::fast());
        let mut writer = BufWriter::new(encoder);

        // to make via swollow this, it neads an xml tag, as well as a dtd header.
        let network_header = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE network SYSTEM \"http://www.matsim.org/files/dtd/network_v2.dtd\">";
        writer.write_all(network_header.as_ref()).unwrap();

        // write the actual network
        to_writer(writer, self).unwrap();

        info!("IONetwork: Finished writing network.");
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::path::PathBuf;

    use quick_xml::de::from_str;

    use crate::simulation::io::network::IONetwork;

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
                    <links>
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
                    <links>
                        <link id=\"23\" from=\"15\" to=\"1\" length=\"10000.00\" capacity=\"36000\" freespeed=\"27.78\" permlanes=\"1\" modes=\"car, bike\" />
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
        assert_eq!(vec!["car", "bike"], link.modes());

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
                    let attr = attrs.attributes.get(0).unwrap();
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
                    let attr = attrs.attributes.get(0).unwrap();
                    assert_eq!("test", attr.name);
                    assert_eq!("value", attr.value);
                }
            }
        }

        println!("{network:#?}");
    }

    #[test]
    fn read_example_file() {
        let file_path = "./assets/equil/equil-network.xml";
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
