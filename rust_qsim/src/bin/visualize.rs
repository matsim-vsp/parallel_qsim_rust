use bevy::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use quick_xml::{events::Event, Reader};
use rust_qsim::generated::events::MyEvent;
use rust_qsim::simulation::io::proto::proto_events::ProtoEventsReader;
use std::collections::HashMap;
use std::io::Cursor;

// Network and Events file paths
const NETWORK_FILE: &str = include_str!("assets/equil-network.xml");
const EVENTS_FILE: &[u8] = include_bytes!("assets/events.0.binpb");

// Defines how much the network coordinates are scaled down for visualization
// TODO: This should be removed and replaced with a correct camera zoom level.
const SCALE_FACTOR: f32 = 1.;

// Defines how much faster the simulation runs compared to the real time
const TIME_SCALE: f32 = 50.0;

// Network node
#[derive(Component, Debug)]
struct Node {
    id: i32,        // node id
    position: Vec2, // node position (x, y)
}

// Network link. The to and from id define the start and end nodes of the link.
#[derive(Component, Debug)]
struct Link {
    #[allow(dead_code)]
    id: i32, // link id
    from_id: i32, // start node id
    to_id: i32,   // end node id
}

// defines all trips and the first start time of all trips
#[derive(Resource)]
struct AllTrips {
    per_vehicle: HashMap<String, Vec<Trip>>, // vehicle id -> trips
    first_start: f32,                        // first start time of all trips
}

// Defines a trip
// TODO: I think this shoulbe be renames. because a trip contains usually multiple links.
#[derive(Clone)]
struct Trip {
    link_id: i32,    // link id
    start_time: f32, // start time
    end_time: f32,   // end time
}

// Clock for the simulation time.
// This clock is independent of the real time provided by Bevy's Time resource.
#[derive(Resource)]
struct SimulationClock {
    time: f32,
}

// network
#[derive(Resource, Default)]
struct NetworkData {
    node_positions: HashMap<i32, Vec2>,       // node id -> position
    link_endpoints: HashMap<i32, (i32, i32)>, // link id -> (from node id, to node id)
}

// This method reads all events from the event file
fn read_events() -> Vec<(u32, Vec<MyEvent>)> {
    ProtoEventsReader::new(Cursor::new(EVENTS_FILE)).collect::<Vec<(u32, Vec<MyEvent>)>>()
}

// This method reads all events and filters them by link enter and leave events
// A trip is then saved from a link enter and the corresponding link leave event.
fn build_vehicle_trips(events: &[(u32, Vec<MyEvent>)]) -> AllTrips {
    // stores on which link a vehicle has been and since when,
    // until the corresponding leave event was found
    // vehicle id -> (link id, start time)
    let mut active: HashMap<String, (i32, f32)> = HashMap::new();
    // stores all trips per vehicle
    let mut per_vehicle: HashMap<String, Vec<Trip>> = HashMap::new();
    // saves the first start time of all trips
    let mut first_start = f32::MAX;

    for (time, events_at_time) in events {
        let time_f = *time as f32;
        for event in events_at_time {
            match event.r#type.as_str() {
                "entered link" => {
                    let link = event.attributes.get("link").map(|v| v.as_string());
                    let vehicle = event.attributes.get("vehicle").map(|v| v.as_string());
                    if let (Some(link), Some(vehicle)) = (link, vehicle) {
                        if let Ok(link_id) = link.parse::<i32>() {
                            active.insert(vehicle, (link_id, time_f));
                        }
                    }
                }
                "left link" => {
                    let link = event.attributes.get("link").map(|v| v.as_string());
                    let vehicle = event.attributes.get("vehicle").map(|v| v.as_string());
                    if let (Some(link), Some(vehicle)) = (link, vehicle) {
                        if let (Ok(link_id), Some((entered_link, start_time))) =
                            (link.parse::<i32>(), active.remove(&vehicle))
                        {
                            if entered_link == link_id && time_f >= start_time {
                                if start_time < first_start {
                                    first_start = start_time;
                                }
                                per_vehicle.entry(vehicle).or_default().push(Trip {
                                    link_id,
                                    start_time,
                                    end_time: time_f,
                                });
                            }
                        }
                    }
                }
                _ => {
                    // All other event types are ignored.
                }
            }
        }
    }

    //set first start to 0 if no trips were found
    if first_start == f32::MAX {
        first_start = 0.0;
    }

    // return all trips
    AllTrips {
        per_vehicle,
        first_start,
    }
}

// This method updates the simulation time based on the real time delta and the timescale.
fn simulation_time(time: Res<Time>, mut clock: ResMut<SimulationClock>) {
    clock.time += time.delta_secs() * TIME_SCALE;
}

fn main() {
    // read all events
    let events = read_events();
    //reate all trips
    let trips = build_vehicle_trips(&events);
    let sim_clock = SimulationClock {
        time: trips.first_start,
    };

    // init bevy app
    App::new()
        .insert_resource(trips)
        .insert_resource(sim_clock)
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
        .add_systems(Update, (simulation_time, draw_network))
        .run();
}

// This method reads and parses the network XML file.
fn read_and_parse_network(mut commands: Commands) {
    let mut reader = Reader::from_str(NETWORK_FILE);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut network = NetworkData::default();

    loop {
        match reader.read_event_into(&mut buf) {
            // each node element provides id and coordinates of a node.
            Ok(Event::Empty(e)) if e.name().as_ref() == b"node" => {
                let mut id: i32 = 0;
                let mut x: f32 = 0.0;
                let mut y: f32 = 0.0;

                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        b"id" => id = a.unescape_value().unwrap().parse().unwrap(),
                        b"x" => x = a.unescape_value().unwrap().parse().unwrap(),
                        b"y" => y = a.unescape_value().unwrap().parse().unwrap(),
                        _ => {}
                    }
                }

                // create a new node entity for bevy
                commands.spawn(Node {
                    id,
                    position: Vec2::new(x, y),
                });
                // store the node position in the network data
                network.node_positions.insert(id, Vec2::new(x, y));
            }

            // each link element provides the connection between two nodes.
            Ok(Event::Empty(e)) if e.name().as_ref() == b"link" => {
                let mut id: i32 = 0;
                let mut from_id: i32 = 0;
                let mut to_id: i32 = 0;

                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        b"id" => id = a.unescape_value().unwrap().parse().unwrap(),
                        b"from" => from_id = a.unescape_value().unwrap().parse().unwrap(),
                        b"to" => to_id = a.unescape_value().unwrap().parse().unwrap(),
                        _ => {}
                    }
                }

                // create a new link entity for bevy
                commands.spawn(Link { id, from_id, to_id });
                // store the link endpoints in the network data
                network.link_endpoints.insert(id, (from_id, to_id));
            }

            Ok(Event::Eof) => break,
            Err(e) => panic!("XML error: {e:?}"),
            _ => {}
        }
        buf.clear();
    }

    commands.insert_resource(network);
}

// creates the camera for visualization
fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, PanCam::default()));
    // commands.spawn((Camera2d));
}

fn draw_network(
    mut gizmos: Gizmos,
    nodes: Query<&Node>,
    links: Query<&Link>,
    trips: Res<AllTrips>,
    network: Res<NetworkData>,
    clock: Res<SimulationClock>,
) {
    // draw the links
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

    // draw the nodes
    for node in &nodes {
        gizmos.circle_2d(
            node.position / SCALE_FACTOR,
            500.0,
            Color::srgb(1.0, 0.0, 0.0),
        );
    }

    // get the current simulation time
    let sim_time = clock.time;

    // loop over all vehicles and draw their current position
    for (_vehicle, trips_for_vehicle) in trips.per_vehicle.iter() {
        // get the current trip for the vehicle at the current simulation time
        if let Some(current_trip) = trips_for_vehicle
            .iter()
            .find(|trip| sim_time >= trip.start_time && sim_time <= trip.end_time)
        {
            // gets the start und end node for the current link
            if let Some(&(from_id, to_id)) = network.link_endpoints.get(&current_trip.link_id) {
                // Get the positions of the from and to nodes.
                if let (Some(&from_pos), Some(&to_pos)) = (
                    network.node_positions.get(&from_id),
                    network.node_positions.get(&to_id),
                ) {
                    // Calculate how long the vehicle is on this link.
                    let duration =
                        (current_trip.end_time - current_trip.start_time).max(f32::EPSILON);

                    // Calculate how far the vehicle has progressed on this link (0 = start, 1 = end).
                    let progress =
                        ((sim_time - current_trip.start_time) / duration).clamp(0.0, 1.0);

                    // Interpolate the position of the vehicle (linear)
                    let position = (from_pos + (to_pos - from_pos) * progress) / SCALE_FACTOR;

                    // Draw the vehicle as a green circle at its current position.
                    gizmos.circle_2d(position, 400.0, Color::srgb(0.0, 1.0, 0.0));
                }
            }
        }
    }
}
