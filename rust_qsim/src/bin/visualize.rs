use bevy::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use quick_xml::{events::Event, Reader};

const NETWORK_FILE: &str = include_str!("assets/equil-network.xml");
const SCALE_FACTOR: f32 = 1.;

#[derive(Component, Debug)]
struct Node {
    id: i32,
    position: Vec2,
}

#[derive(Component, Debug)]
struct Link {
    #[allow(dead_code)]
    id: i32,
    from_id: i32,
    to_id: i32,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Network Viewer".into(),
                    resolution: (1200, 800).into(),
                    resizable: true,
                    ..default()
                }),
                ..default()
            }),
            PanCamPlugin::default(),
        ))
        .add_systems(Startup, (read_and_parse_network, setup).chain())
        .add_systems(Update, draw_network)
        .run();
}

// Read and parse the network XML file
fn read_and_parse_network(mut commands: Commands) {
    let mut reader = Reader::from_str(NETWORK_FILE);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            // Check if the event is a <node .../> element.
            Ok(Event::Empty(e)) if e.name().as_ref() == b"node" => {
                let mut id: i32 = 0;
                let mut x: f32 = 0.0;
                let mut y: f32 = 0.0;

                // Read attributes: id, x and y
                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        b"id" => id = a.unescape_value().unwrap().parse().unwrap(),
                        b"x" => x = a.unescape_value().unwrap().parse().unwrap(),
                        b"y" => y = a.unescape_value().unwrap().parse().unwrap(),
                        _ => {}
                    }
                }

                commands.spawn(Node {
                    id,
                    position: Vec2::new(x, y),
                });
            }

            // Check if the event is a <link .../> element.
            Ok(Event::Empty(e)) if e.name().as_ref() == b"link" => {
                let mut id: i32 = 0;
                let mut from_id: i32 = 0;
                let mut to_id: i32 = 0;

                // Read attributes: id, from and to.
                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        b"id" => id = a.unescape_value().unwrap().parse().unwrap(),
                        b"from" => from_id = a.unescape_value().unwrap().parse().unwrap(),
                        b"to" => to_id = a.unescape_value().unwrap().parse().unwrap(),
                        _ => {}
                    }
                }

                commands.spawn(Link { id, from_id, to_id });
            }

            Ok(Event::Eof) => break,
            Err(e) => panic!("XML error: {e:?}"),
            _ => {}
        }
        buf.clear();
    }
}

// Sets the camera and adding pancam for zoom control
fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, PanCam::default()));
    // commands.spawn((Camera2d));
}

fn draw_network(mut gizmos: Gizmos, nodes: Query<&Node>, links: Query<&Link>) {
    // Draw links
    for link in &links {
        let from_node = nodes.iter().find(|n| n.id == link.from_id);
        let to_node = nodes.iter().find(|n| n.id == link.to_id);

        if let (Some(from), Some(to)) = (from_node, to_node) {
            gizmos.line_2d(
                from.position / SCALE_FACTOR,
                to.position / SCALE_FACTOR,
                Color::srgb(1.0, 1.0, 1.0),
            );
        }
    }

    // Draw nodes
    for node in &nodes {
        gizmos.circle_2d(
            node.position / SCALE_FACTOR,
            500.0,
            Color::srgb(1.0, 0.0, 0.0),
        );
    }
}
