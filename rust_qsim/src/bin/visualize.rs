#![allow(dead_code)]

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_pancam::{PanCam, PanCamPlugin};
use quick_xml::{events::Event, Reader};
use rust_qsim::generated::events::MyEvent;
use rust_qsim::simulation::io::proto::proto_events::ProtoEventsReader;
use std::collections::HashMap;
use std::io::Cursor;

// Network and Events file paths
const NETWORK_FILE: &str = include_str!("assets/equil-network.xml");
const VEHICLES_FILE: &str = include_str!("assets/equil-vehicles.xml");
const EVENTS_FILE: &[u8] = include_bytes!("assets/events.0.binpb");

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
    id: i32,        // link id
    from_id: i32,   // start node id
    to_id: i32,     // end node id
    freespeed: f32, // free flow speed on link [m/s]
}

// defines all trips and the first start time of all trips
#[derive(Resource)]
struct AllTrips {
    per_vehicle: HashMap<String, Vec<TraversedLink>>, // vehicle id -> trips
    first_start: f32,                                  // first start time of all trips
}

// Defines a traversed link: a vehicle moving along a single link during a time interval.
#[derive(Clone)]
struct TraversedLink {
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
    link_freespeed: HashMap<i32, f32>,        // link id -> freespeed
}

#[derive(Resource)]
struct ViewSettings {
    center: Vec2,
    scale: f32,
}

#[derive(Debug, Clone)]
struct Vehicle {
    id: String,
    maximum_velocity: f32, // maximum vehicle speed [m/s]
}

#[derive(Resource, Default)]
struct VehiclesData {
    vehicles: HashMap<String, Vehicle>,
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
    let mut per_vehicle: HashMap<String, Vec<TraversedLink>> = HashMap::new();
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
                                per_vehicle.entry(vehicle).or_default().push(TraversedLink {
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

// TODO: Check what happen if i use two clocks e.g. or to networks (two resources)
// two ressources point tzo the same data and modify the data (+1 and -1) and check what happens
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
        .add_systems(
            Startup,
            (
                read_and_parse_network,
                read_and_parse_vehicles,
                fit_camera_to_network,
                setup,
            )
                .chain(),
        )
        .add_systems(Update, (simulation_time, draw_network))
        .run();
}

// This method reads and parses the network XML file.
// Reüplace with protobuf instwad of xml
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
                let mut freespeed: f32 = 0.0;

                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        b"id" => id = a.unescape_value().unwrap().parse().unwrap(),
                        b"from" => from_id = a.unescape_value().unwrap().parse().unwrap(),
                        b"to" => to_id = a.unescape_value().unwrap().parse().unwrap(),
                        b"freespeed" => freespeed = a.unescape_value().unwrap().parse().unwrap(),
                        _ => {}
                    }
                }

                // create a new link entity for bevy
                commands.spawn(Link {
                    id,
                    from_id,
                    to_id,
                    freespeed,
                });
                // store the link endpoints in the network data
                network.link_endpoints.insert(id, (from_id, to_id));
                network.link_freespeed.insert(id, freespeed);
            }

            Ok(Event::Eof) => break,
            Err(e) => panic!("XML error: {e:?}"),
            _ => {}
        }
        buf.clear();
    }

    commands.insert_resource(network);
}

fn read_and_parse_vehicles(mut commands: Commands) {
    let mut reader = Reader::from_str(VEHICLES_FILE);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut vehicles: HashMap<String, Vehicle> = HashMap::new();
    let mut current_vehicle_id: Option<String> = None;
    let mut pending_velocity_vehicle: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"vehicleType" => {
                let mut vehicle_id = None;
                for a in e.attributes().flatten() {
                    if a.key.as_ref() == b"id" {
                        vehicle_id = Some(a.unescape_value().unwrap().into_owned());
                    }
                }
                current_vehicle_id = vehicle_id;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"vehicleType" => {
                current_vehicle_id = None;
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"maximumVelocity" => {
                if let Some(vehicle_id) = current_vehicle_id.as_ref() {
                    let mut velocity: Option<f32> = None;
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == b"meterPerSecond" {
                            velocity = Some(a.unescape_value().unwrap().parse().unwrap());
                        }
                    }
                    if let Some(speed) = velocity {
                        vehicles.insert(
                            vehicle_id.clone(),
                            Vehicle {
                                id: vehicle_id.clone(),
                                maximum_velocity: speed,
                            },
                        );
                    }
                }
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"maximumVelocity" => {
                pending_velocity_vehicle = current_vehicle_id.clone();
                if let Some(vehicle_id) = pending_velocity_vehicle.as_ref() {
                    let mut velocity: Option<f32> = None;
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == b"meterPerSecond" {
                            velocity = Some(a.unescape_value().unwrap().parse().unwrap());
                        }
                    }
                    if let Some(speed) = velocity {
                        vehicles.insert(
                            vehicle_id.clone(),
                            Vehicle {
                                id: vehicle_id.clone(),
                                maximum_velocity: speed,
                            },
                        );
                        pending_velocity_vehicle = None;
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(vehicle_id) = pending_velocity_vehicle.as_ref() {
                    if let Ok(value) = e.unescape() {
                        if let Ok(speed) = value.parse::<f32>() {
                            vehicles.insert(
                                vehicle_id.clone(),
                                Vehicle {
                                    id: vehicle_id.clone(),
                                    maximum_velocity: speed,
                                },
                            );
                            pending_velocity_vehicle = None;
                        }
                    }
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"maximumVelocity" => {
                pending_velocity_vehicle = None;
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML error while parsing vehicles: {e:?}"),
            _ => {}
        }
        buf.clear();
    }

    commands.insert_resource(VehiclesData { vehicles });
}

// creates the camera for visualization
fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, PanCam::default()));
    // commands.spawn((Camera2d));
}

// This method inspects the loaded network and window size
// and computes a view center and zoom level so that the whole network is visible.
fn fit_camera_to_network(
    mut commands: Commands,
    network: Option<Res<NetworkData>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    // If the network resource does not exist yet, we cannot compute a view.
    let Some(network) = network else {
        return;
    };

    // If there are no nodes, there is nothing to fit.
    if network.node_positions.is_empty() {
        return;
    }

    // Use the primary window to know how much space we have on screen.
    let Some(window) = window_query.iter().next() else {
        return;
    };

    // Determine the bounding box of all node positions.
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for position in network.node_positions.values() {
        min_x = min_x.min(position.x);
        max_x = max_x.max(position.x);
        min_y = min_y.min(position.y);
        max_y = max_y.max(position.y);
    }

    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return;
    }

    // Compute network width, height and geometric center.
    let width = (max_x - min_x).max(f32::EPSILON);
    let height = (max_y - min_y).max(f32::EPSILON);

    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;

    // Relate network extent to the current window size to derive a zoom level.
    let window_width = window.width().max(1.0);
    let window_height = window.height().max(1.0);

    let required_scale_x = width / window_width;
    let required_scale_y = height / window_height;

    // Add a small margin so the network is not exactly at the border.
    let margin = 1.1;
    let mut scale = required_scale_x.max(required_scale_y) * margin;
    if !scale.is_finite() || scale <= 0.0 {
        scale = 1.0;
    }

    commands.insert_resource(ViewSettings {
        center: Vec2::new(center_x, center_y),
        scale,
    });
}

fn draw_network(
    mut gizmos: Gizmos,
    nodes: Query<&Node>,
    links: Query<&Link>,
    trips: Res<AllTrips>,
    network: Res<NetworkData>,
    view: Option<Res<ViewSettings>>,
    clock: Res<SimulationClock>,
) {
    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

    // draw the links
    for link in &links {
        let from_node = nodes.iter().find(|n| n.id == link.from_id);
        let to_node = nodes.iter().find(|n| n.id == link.to_id);

        if let (Some(from), Some(to)) = (from_node, to_node) {
            gizmos.line_2d(
                (from.position - center) / scale,
                (to.position - center) / scale,
                Color::srgb(1.0, 1.0, 1.0),
            );
        }
    }

    // draw the nodes
    for node in &nodes {
        gizmos.circle_2d(
            (node.position - center) / scale,
            4.0,
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
                    let position = (from_pos + (to_pos - from_pos) * progress - center) / scale;

                    // Draw the vehicle as a green circle at its current position.
                    gizmos.circle_2d(position, 4.0, Color::srgb(0.0, 1.0, 0.0));
                }
            }
        }
    }
}

// TODO: Next possiblöe steps
//
