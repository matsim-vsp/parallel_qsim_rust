use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_pancam::{PanCam, PanCamPlugin};
use prost::Message;
use rust_qsim::generated::events::MyEvent;
use rust_qsim::generated::general::AttributeValue;
use rust_qsim::generated::network as wire_network;
use rust_qsim::generated::vehicles as wire_vehicles;
use rust_qsim::simulation::events::*;
use rust_qsim::simulation::events::{EventTrait, EventsPublisher, PtTeleportationArrivalEvent};
use rust_qsim::simulation::io::proto::proto_events::ProtoEventsReader;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::rc::Rc;

// equil scenario
const NETWORK_FILE: &[u8] = include_bytes!("assets/equil/equil-network.binpb");
const VEHICLES_FILE: &[u8] = include_bytes!("assets/equil/equil-vehicles.binpb");
const EVENTS_FILE: &[u8] = include_bytes!("assets/equil/events.0.binpb");

// Defines how much faster the simulation runs compared to the real time
const TIME_SCALE: f32 = 50.0;

// Network node
#[derive(Component, Debug)]
struct Node {
    id: String,     // node id
    position: Vec2, // node position (x, y)
}

// Network link
#[derive(Component, Debug)]
struct Link {
    from_id: String, // start node id
    to_id: String,   // end node id
}

// defines all trips and the first start time of all trips
#[derive(Resource)]
struct AllTrips {
    per_vehicle: HashMap<String, Vec<TraversedLink>>, // vehicle id -> trips
    first_start: f32,                                 // first start time of all trips
}

// Defines a traversed link
#[derive(Clone)]
struct TraversedLink {
    link_id: String, // link id
    start_time: f32, // start time
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
    node_positions: HashMap<String, Vec2>, // node id -> position
    link_endpoints: HashMap<String, (String, String)>, // link id -> (from node id, to node id)
    link_freespeed: HashMap<String, f32>,  // link id -> freespeed
}

// view settings
#[derive(Resource)]
struct ViewSettings {
    center: Vec2,
    scale: f32,
}

// vehicle
#[derive(Debug, Clone)]
struct Vehicle {
    maximum_velocity: f32, // maximum vehicle speed [m/s]
}

#[derive(Resource, Default)]
struct VehiclesData {
    vehicles: HashMap<String, Vehicle>,
}

#[derive(Default)]
struct TripsBuilder {
    // stores the current link of a vehicle
    current_link_per_vehicle: HashMap<String, (String, f32)>,
    // stores all traversed links per vehicle
    per_vehicle: HashMap<String, Vec<TraversedLink>>,
    // Earliest start time of all vehicles
    first_start: f32,
}

impl TripsBuilder {
    // Creates a new TripsBuilder
    fn new() -> Self {
        Self {
            current_link_per_vehicle: HashMap::new(),
            per_vehicle: HashMap::new(),
            first_start: f32::MAX,
        }
    }

    // check if an event is a LinkEnter or a LinkLeaveEvent and calls the corresponding methode
    fn handle_event(&mut self, event: &dyn EventTrait) {
        if let Some(enter) = event.as_any().downcast_ref::<LinkEnterEvent>() {
            self.handle_link_enter(enter);
        } else if let Some(leave) = event.as_any().downcast_ref::<LinkLeaveEvent>() {
            self.handle_link_leave(leave);
        }
    }

    // Handles a link enter event and remembers which vehicle entered which link at what time.
    fn handle_link_enter(&mut self, event: &LinkEnterEvent) {
        let link_id = event.link.external().to_string();
        let vehicle_id = event.vehicle.external().to_string();
        self.current_link_per_vehicle
            .insert(vehicle_id, (link_id, event.time as f32));
    }

    // Handles a link leave event by closing the currently active traversed link for the vehicle.
    fn handle_link_leave(&mut self, event: &LinkLeaveEvent) {
        let link_id = event.link.external().to_string();
        let vehicle_id = event.vehicle.external().to_string();
        if let Some((entered_link, start_time)) = self.current_link_per_vehicle.remove(&vehicle_id)
        {
            let end_time = event.time as f32;
            if entered_link == link_id && end_time >= start_time {
                if start_time < self.first_start {
                    self.first_start = start_time;
                }
                self.per_vehicle
                    .entry(vehicle_id)
                    .or_default()
                    .push(TraversedLink {
                        link_id,
                        start_time,
                    });
            }
        }
    }

    // Builds the AllTrips from the collected traversed links.
    fn build_all_trips(&self) -> AllTrips {
        // Clone trips so we can sort them per vehicle by start time
        let mut per_vehicle = self.per_vehicle.clone();
        for trips in per_vehicle.values_mut() {
            trips.sort_by(|a, b| {
                a.start_time
                    .partial_cmp(&b.start_time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let mut first_start = self.first_start;
        // Set first start to 0 if no traversed links were found
        if first_start == f32::MAX {
            first_start = 0.0;
        }
        AllTrips {
            per_vehicle,
            first_start,
        }
    }
}

// Registers a listener on the EventsPublisher that dends all events to the TripsBuilder
fn register_trips_listener(builder: Rc<RefCell<TripsBuilder>>, publisher: &mut EventsPublisher) {
    // Create a clone of the TripsBuilder
    let builder_for_events = builder.clone();

    // Send every published event to ther TripsBuilder
    publisher.on_any(move |event| {
        builder_for_events.borrow_mut().handle_event(event);
    });
}

// This method reads all proto events, sends them through the EventsPublisher,
// and collects all traversed links per vehicle.
fn build_vehicle_trips() -> AllTrips {
    // Reader for all ProtoEvents
    let reader = ProtoEventsReader::new(Cursor::new(EVENTS_FILE));

    // Create a new TripsBuilder to collect all traversed links
    let builder = Rc::new(RefCell::new(TripsBuilder::new()));

    // Create a new EventsPublisher and connect it to the TripsBuilder
    let mut publisher = EventsPublisher::new();
    register_trips_listener(builder.clone(), &mut publisher);

    // Iterate over all events
    for (time, events_at_time) in reader {
        process_events(time, &events_at_time, &mut publisher);
    }

    publisher.finish();

    // Build the AllTrips from the collected events
    let trips = builder.borrow().build_all_trips();
    trips
}

// This method updates the simulation time based on the real time delta and the timescale.
fn simulation_time(time: Res<Time>, mut clock: ResMut<SimulationClock>) {
    clock.time += time.delta_secs() * TIME_SCALE;
}

#[rustfmt::skip]
fn process_events(time: u32, events: &Vec<MyEvent>, publisher: &mut EventsPublisher) {
    for proto_event in events {
        let mut proto_event = proto_event.clone();
        if !proto_event.attributes.contains_key("type") {
            proto_event
                .attributes
                .insert("type".to_string(), AttributeValue::from(proto_event.r#type.as_str()));
        }

        let type_ = proto_event.attributes["type"].as_string();
        let internal_event: Box<dyn EventTrait> = match type_.as_str() {
            GeneralEvent::TYPE => Box::new(GeneralEvent::from_proto_event(&proto_event, time)),
            ActivityStartEvent::TYPE => Box::new(ActivityStartEvent::from_proto_event(&proto_event, time)),
            ActivityEndEvent::TYPE => Box::new(ActivityEndEvent::from_proto_event(&proto_event, time)),
            LinkEnterEvent::TYPE => Box::new(LinkEnterEvent::from_proto_event(&proto_event, time)),
            LinkLeaveEvent::TYPE => Box::new(LinkLeaveEvent::from_proto_event(&proto_event, time)),
            PersonEntersVehicleEvent::TYPE => Box::new(PersonEntersVehicleEvent::from_proto_event(&proto_event, time)),
            PersonLeavesVehicleEvent::TYPE => Box::new(PersonLeavesVehicleEvent::from_proto_event(&proto_event, time)),
            PersonDepartureEvent::TYPE => Box::new(PersonDepartureEvent::from_proto_event(&proto_event, time)),
            PersonArrivalEvent::TYPE => Box::new(PersonArrivalEvent::from_proto_event(&proto_event, time)),
            TeleportationArrivalEvent::TYPE => Box::new(TeleportationArrivalEvent::from_proto_event(&proto_event, time)),
            PtTeleportationArrivalEvent::TYPE => Box::new(PtTeleportationArrivalEvent::from_proto_event(&proto_event, time)),
            _ => panic!("Unknown event type: {:?}", type_),
        };
        publisher.publish_event(internal_event.as_ref());
    }
}

fn main() {
    // read all events and build traversed links per vehicle
    let trips = build_vehicle_trips();

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
        .add_systems(Update, (simulation_time, draw_network, draw_vehicles))
        .run();
}

// This method reads and parses the network protobuf file.
fn read_and_parse_network(mut commands: Commands) {
    // decode the protobuf network from the embedded bytes
    let wire: wire_network::Network =
        wire_network::Network::decode(NETWORK_FILE).expect("Failed to decode network protobuf");
    let mut network = NetworkData::default();

    // each protobuf node provides id and coordinates of a node.
    for wn in &wire.nodes {
        let id = wn.id.clone();
        let x: f32 = wn.x as f32;
        let y: f32 = wn.y as f32;

        // create a new node entity for bevy
        commands.spawn(Node {
            id: id.clone(),
            position: Vec2::new(x, y),
        });
        // store the node position in the network data
        network.node_positions.insert(id, Vec2::new(x, y));
    }

    // each protobuf link provides the connection between two nodes.
    for wl in &wire.links {
        let id = wl.id.clone();
        let from_id = wl.from.clone();
        let to_id = wl.to.clone();
        let freespeed: f32 = wl.freespeed;

        // create a new link entity for bevy
        commands.spawn(Link {
            from_id: from_id.clone(),
            to_id: to_id.clone(),
        });
        // store the link endpoints in the network data
        network.link_endpoints.insert(id, (from_id, to_id));
        network.link_freespeed.insert(wl.id.clone(), freespeed);
    }

    commands.insert_resource(network);
}

fn read_and_parse_vehicles(mut commands: Commands) {
    // decode the protobuf vehicles container from the embedded bytes
    let wire: wire_vehicles::VehiclesContainer =
        wire_vehicles::VehiclesContainer::decode(VEHICLES_FILE)
            .expect("Failed to decode vehicles protobuf");

    let mut vehicles: HashMap<String, Vehicle> = HashMap::new();

    // Each protobuf vehicle provides id and maximum velocity.
    for wv in &wire.vehicles {
        let id = wv.id.to_string();
        let maximum_velocity = wv.max_v as f32;

        // Store the vehicle by its id together with its maximum speed.
        vehicles.insert(id.clone(), Vehicle { maximum_velocity });
    }

    commands.insert_resource(VehiclesData { vehicles });
}

// creates the camera for visualization
fn setup(mut commands: Commands) {
    // Spawn a simple 2D camera that supports panning and zooming via PanCam.
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
    view: Option<Res<ViewSettings>>,
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
}

fn draw_vehicles(
    mut gizmos: Gizmos,
    trips: Res<AllTrips>,
    network: Res<NetworkData>,
    vehicles: Res<VehiclesData>,
    view: Option<Res<ViewSettings>>,
    clock: Res<SimulationClock>,
) {
    // Use current view settings (center/scale) or fall back to defaults.
    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

    // Get the current simulation time.
    let sim_time = clock.time;

    // Loop over all vehicles and draw their current position.
    for (vehicle_id, trips_for_vehicle) in trips.per_vehicle.iter() {
        if trips_for_vehicle.is_empty() {
            continue;
        }

        // Maximum vehicle speed; if unknown, assume no additional limit.
        let vehicle_v_max = vehicles
            .vehicles
            .get(vehicle_id)
            .map(|v| v.maximum_velocity)
            .unwrap_or(f32::INFINITY);

        let mut position_to_draw: Option<Vec2> = None;
        let mut prev_arrival_time: Option<f32> = None;
        let mut prev_arrival_pos: Option<Vec2> = None;

        for trip in trips_for_vehicle {
            // Get link endpoints (from and to node ids) from the NetworkData resource.
            let (from_id, to_id) = match network.link_endpoints.get(&trip.link_id) {
                Some(v) => v.clone(),
                None => continue,
            };

            // Get the positions of the from and to nodes.
            let (from_pos, to_pos) = match (
                network.node_positions.get(&from_id),
                network.node_positions.get(&to_id),
            ) {
                (Some(&from), Some(&to)) => (from, to),
                _ => continue,
            };

            // Length of the link (in meters) based on node coordinates.
            let link_vector = to_pos - from_pos;
            let link_length = link_vector.length().max(f32::EPSILON);

            // Maximum allowed speed on this link from the network.
            let link_v_max = *network
                .link_freespeed
                .get(&trip.link_id)
                .unwrap_or(&f32::INFINITY);

            // Effective driving speed: limited by both link and vehicle.
            let v_eff = vehicle_v_max.min(link_v_max);
            if v_eff <= 0.0 {
                // cannot move with non-positive speed
                continue;
            }

            // Travel time on this link with the effective speed.
            let travel_duration = link_length / v_eff;

            let scheduled_start = trip.start_time;

            // Actual departure time: not before scheduled_start and not before previous arrival.
            let depart_time = match prev_arrival_time {
                Some(arrival_prev) => {
                    // If we arrived earlier than the scheduled start of this link,
                    // the vehicle waits at the previous node until scheduled_start.
                    let depart = scheduled_start.max(arrival_prev);
                    if sim_time >= arrival_prev && sim_time < depart {
                        if let Some(wait_pos) = prev_arrival_pos {
                            position_to_draw = Some(wait_pos);
                            break;
                        }
                    }
                    depart
                }
                None => scheduled_start,
            };

            let arrival_time = depart_time + travel_duration;

            // If the vehicle is currently on this link, interpolate position.
            if sim_time >= depart_time && sim_time < arrival_time {
                let progress = ((sim_time - depart_time) / travel_duration).clamp(0.0, 1.0);
                let position = from_pos + link_vector * progress;
                position_to_draw = Some(position);
                break;
            }

            // Prepare for the next link: remember where and when we arrived.
            prev_arrival_time = Some(arrival_time);
            prev_arrival_pos = Some(to_pos);
        }

        // If we have already finished the last link, keep the vehicle waiting at the last node.
        if position_to_draw.is_none() {
            if let (Some(arrival_time), Some(arrival_pos)) = (prev_arrival_time, prev_arrival_pos) {
                if sim_time >= arrival_time {
                    position_to_draw = Some(arrival_pos);
                }
            }
        }

        if let Some(position_world) = position_to_draw {
            let position_view = (position_world - center) / scale;
            gizmos.circle_2d(position_view, 4.0, Color::srgb(0.0, 1.0, 0.0));
        }
    }
}
