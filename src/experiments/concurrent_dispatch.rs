use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

fn run() {
    let mut nodes = Vec::new();

    for n in 0..5 {
        let channel = mpsc::channel();
        let id = nodes.len();
        let node = Node::new(id, channel.1);
        nodes.push((node, channel.0));
    }

    let mut connections = Vec::new();
    for mut tuple in nodes.iter_mut() {
        let node = &tuple.0;
        let sender = tuple.1.clone();
        let connection = Connection {
            target_id: 2,
            sender,
        };
        connections.push((node.id, connection));
    }

    for connection in connections {
        nodes.get(connection.0)
    }
}

struct Node {
    id: usize,
    receiver: Receiver<Message>,
    connections: Vec<Connection>,
}

impl Node {
    fn new(id: usize, receiver: Receiver<Message>) -> Self {
        Self {
            id,
            receiver,
            connections: Vec::new(),
        }
    }
}

struct Connection {
    target_id: usize,
    sender: Sender<Message>,
}

struct Message {}
