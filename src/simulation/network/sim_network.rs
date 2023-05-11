use std::collections::HashMap;

use crate::simulation::id::Id;

use super::global_network::{Node, Network};

pub struct SimNetwork<'n> {
    nodes: Vec<Id<Node>>,
    links: HashMap<Id<super::global_network::Link>, super::link::Link>,
    global_network: &'n Network<'n>,
}

impl<'n> SimNetwork<'n> {
    
    
    pub fn move_nodes(&mut self) {
        
        
        
        
        
        for node_id in &self.nodes {
            let node = self.global_network.get_node(&node_id);
            for link_id in &node.in_links {
                let link = self.links.get_mut(link_id).unwrap();
                match link {
                    super::link::Link::LocalLink(_) => todo!(),
                    super::link::Link::SplitInLink(_) => todo!(),
                    super::link::Link::SplitOutLink(_) => todo!(),
                }
            }
            
        }
    }
    
    fn move_node(&mut self, node: &Node) {
        
    }
}
