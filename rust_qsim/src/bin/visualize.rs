use bevy::prelude::*;
use bevy::ui::Node as UiNode;
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
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

// ============================================================================
// CONSTANTS & CONFIGURATION
// ============================================================================

// equil scenario
// const NETWORK_FILE: &[u8] = include_bytes!("assets/equil/equil-network.binpb");
// const VEHICLES_FILE: &[u8] = include_bytes!("assets/equil/equil-vehicles.binpb");
// const EVENTS_FILE: &[u8] = include_bytes!("assets/equil/events.0.binpb");

const NETWORK_FILE: &[u8] = include_bytes!("assets/equil-100/equil-100-network.binpb");
const VEHICLES_FILE: &[u8] = include_bytes!("assets/equil-100/equil-100-vehicles.binpb");
const EVENTS_FILE: &[u8] = include_bytes!("assets/equil-100/events.0.binpb");

// Defines how much faster the simulation runs compared to the real time
const TIME_SCALE: f32 = 50.0;

const WAIT_STACK_OFFSET: f32 = 8.0;

// defines how often the time and the fps value should be updated.
// if the value is to small the fps value is not readable
const TIME_FPS_UPDATE_EVERY_N_FRAMES: u32 = 50;

// defines how often trips should be updated from incoming events (in frames)
const TRIPS_UPDATE_EVERY_N_FRAMES: u32 = 10;

// ============================================================================
// DATA STRUCTURES & RESOURCES
// ============================================================================

// Defines a traversed link
#[derive(Clone)]
struct TraversedLink {
    link_id: String, // link id
    start_time: f32, // start time
}

// Defines a single trip of a vehicle (sequence of traversed links)
#[derive(Clone)]
struct Trip {
    links: Vec<TraversedLink>,
}

// defines all trips and the first start time of all trips
#[derive(Resource)]
struct AllTrips {
    per_vehicle: HashMap<String, Vec<Trip>>, // vehicle id -> trips
    first_start: f32,                        // first start time of all trips
}

// Clock for the simulation time.
// This clock is independent of the real time provided by Bevy's Time resource.
#[derive(Resource)]
struct SimulationClock {
    time: f32,
}

// Resource that contains the receiver side of the event channel.
#[derive(Resource)]
struct EventsChannel {
    receiver: Mutex<mpsc::Receiver<Vec<MyEvent>>>,
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

// ui
#[derive(Component)]
struct TimeFpsText;

#[derive(Resource, Default)]
struct VehiclesData {
    vehicles: HashMap<String, Vehicle>,
}

// Shared state for the TripsBuilder that can be accessed from both the thread and Bevy systems
#[derive(Resource, Clone)]
struct SharedTripsBuilder {
    builder: Arc<Mutex<TripsBuilder>>,
}

// ============================================================================
// TRIPS BUILDER
// ============================================================================

#[derive(Default)]
struct TripsBuilder {
    // stores the current link of a vehicle
    current_link_per_vehicle: HashMap<String, (String, f32)>,
    // stores the currently active trip per vehicle (sequence of traversed links)
    current_trip_per_vehicle: HashMap<String, Vec<TraversedLink>>,
    // stores all finished trips per vehicle
    per_vehicle: HashMap<String, Vec<Trip>>,
    // Earliest start time of all vehicles
    first_start: f32,
}

impl TripsBuilder {
    // create a new TripsBuilder
    fn new() -> Self {
        Self {
            current_link_per_vehicle: HashMap::new(),
            current_trip_per_vehicle: HashMap::new(),
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
        } else if let Some(enter) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            self.handle_person_enters(enter);
        } else if let Some(leave) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            self.handle_person_leaves(leave);
        }
    }

    // handles a link enter event and remembers which vehicle entered which link at what time
    fn handle_link_enter(&mut self, event: &LinkEnterEvent) {
        let link_id = event.link.external().to_string();
        let vehicle_id = event.vehicle.external().to_string();
        self.current_link_per_vehicle
            .insert(vehicle_id, (link_id, event.time as f32));
    }

    // handles a link leave event by closing the currently active traversed link for the vehicle
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
                // only record links that belong to an active trip
                if let Some(current_trip) = self.current_trip_per_vehicle.get_mut(&vehicle_id) {
                    current_trip.push(TraversedLink {
                        link_id,
                        start_time,
                    });
                }
            }
        }
    }

    // handles a person enters vehicle event and starts a new trip (if not already active)
    fn handle_person_enters(&mut self, event: &PersonEntersVehicleEvent) {
        let vehicle_id = event.vehicle.external().to_string();
        self.current_trip_per_vehicle
            .entry(vehicle_id)
            .or_insert_with(Vec::new);
    }

    // handles a person leaves vehicle event and finishes the current trip
    fn handle_person_leaves(&mut self, event: &PersonLeavesVehicleEvent) {
        let vehicle_id = event.vehicle.external().to_string();
        if let Some(trip_links) = self.current_trip_per_vehicle.remove(&vehicle_id) {
            if !trip_links.is_empty() {
                self.per_vehicle
                    .entry(vehicle_id)
                    .or_default()
                    .push(Trip { links: trip_links });
            }
        }
    }

    // build AllTrips from the traversed links
    fn build_all_trips(&self) -> AllTrips {
        // clone finished trips and also include currently active (but not yet closed) trips
        let mut per_vehicle: HashMap<String, Vec<Trip>> = HashMap::new();

        for (veh_id, trips) in &self.per_vehicle {
            per_vehicle.insert(veh_id.clone(), trips.clone());
        }

        for (veh_id, current_links) in &self.current_trip_per_vehicle {
            if !current_links.is_empty() {
                per_vehicle.entry(veh_id.clone()).or_default().push(Trip {
                    links: current_links.clone(),
                });
            }
        }

        // sort links inside each trip and the trips themselves by start time
        for trips in per_vehicle.values_mut() {
            for trip in trips.iter_mut() {
                trip.links.sort_by(|a, b| {
                    a.start_time
                        .partial_cmp(&b.start_time)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            trips.sort_by(|a, b| {
                let a_start = a.links.first().map(|t| t.start_time).unwrap_or(f32::MAX);
                let b_start = b.links.first().map(|t| t.start_time).unwrap_or(f32::MAX);
                a_start
                    .partial_cmp(&b_start)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let mut first_start = self.first_start;
        // set first start to 0 if no traversed link was found
        if first_start == f32::MAX {
            first_start = 0.0;
        }
        // save all trips in AllTrips
        AllTrips {
            per_vehicle,
            first_start,
        }
    }
}

// ============================================================================
// EVENT PROCESSING & THREADING
// ============================================================================

// registers a listener on the EventsPublisher that sends all events to the TripsBuilder
fn register_trips_listener(builder: Rc<RefCell<TripsBuilder>>, publisher: &mut EventsPublisher) {
    // create a clone of the TripsBuilder
    let builder_for_events = builder.clone();

    // send every published event to the TripsBuilder
    publisher.on_any(move |event| {
        builder_for_events.borrow_mut().handle_event(event);
    });
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

// This method starts a new thread that reads all events from the proto events file and
// processes them in real-time, updating the shared TripsBuilder
fn start_events_thread(shared_builder: SharedTripsBuilder) -> EventsChannel {
    // create channel for sending events to visualization
    let (tx_viz, rx_viz) = mpsc::channel::<Vec<MyEvent>>();

    // create a new thread for reading events
    thread::spawn(move || {
        // reader for all proto events
        let reader = ProtoEventsReader::new(Cursor::new(EVENTS_FILE));

        // create a local TripsBuilder that feeds into the shared one
        let builder = Rc::new(RefCell::new(TripsBuilder::new()));
        let mut publisher = EventsPublisher::new();
        register_trips_listener(builder.clone(), &mut publisher);

        // iterate over all events and process them
        for (time, events_at_time) in reader {
            // process events locally
            process_events(time, &events_at_time, &mut publisher);

            // send to visualization channel
            if tx_viz.send(events_at_time).is_err() {
                break;
            }

            // periodically update the shared builder with current state
            if let Ok(mut shared) = shared_builder.builder.lock() {
                *shared = builder.borrow().clone();
            }
        }

        publisher.finish();

        // final update with complete data
        if let Ok(mut shared) = shared_builder.builder.lock() {
            *shared = builder.borrow().clone();
        }
    });

    EventsChannel {
        receiver: Mutex::new(rx_viz),
    }
}

// Clone implementation for TripsBuilder
impl Clone for TripsBuilder {
    fn clone(&self) -> Self {
        Self {
            current_link_per_vehicle: self.current_link_per_vehicle.clone(),
            current_trip_per_vehicle: self.current_trip_per_vehicle.clone(),
            per_vehicle: self.per_vehicle.clone(),
            first_start: self.first_start,
        }
    }
}

// This system receives all events from the event channel (currently just drains them)
fn receive_events_from_channel(events_channel: Res<EventsChannel>) {
    if let Ok(receiver) = events_channel.receiver.lock() {
        while let Ok(_timed_events) = receiver.try_recv() {}
    }
}

// This system periodically updates the AllTrips resource from the shared builder
fn update_trips_from_builder(
    mut frame_counter: Local<u32>,
    mut clock_initialized: Local<bool>,
    shared_builder: Res<SharedTripsBuilder>,
    mut trips: ResMut<AllTrips>,
    mut clock: ResMut<SimulationClock>,
) {
    *frame_counter += 1;
    if *frame_counter % TRIPS_UPDATE_EVERY_N_FRAMES != 0 {
        return;
    }

    // update trips from the shared builder
    if let Ok(builder) = shared_builder.builder.lock() {
        let new_trips = builder.build_all_trips();

        // on first update: set clock to the first event time
        if !*clock_initialized && new_trips.first_start > 0.0 && new_trips.first_start != f32::MAX {
            clock.time = new_trips.first_start;
            *clock_initialized = true;
        }

        *trips = new_trips;
    }
}

// ============================================================================
// DATA LOADING (Network & Vehicles)
// ============================================================================

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

        // store the node position in the network data
        network.node_positions.insert(id, Vec2::new(x, y));
    }

    // each protobuf link provides the connection between two nodes.
    for wl in &wire.links {
        let id = wl.id.clone();
        let from_id = wl.from.clone();
        let to_id = wl.to.clone();
        let freespeed: f32 = wl.freespeed;

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
        let maximum_velocity = wv.max_v;

        // Store the vehicle by its id together with its maximum speed.
        vehicles.insert(id.clone(), Vehicle { maximum_velocity });
    }

    commands.insert_resource(VehiclesData { vehicles });
}

// ============================================================================
// SIMULATION TIME
// ============================================================================

// this method updates the simulation time based on the real time delta and the timescale.
fn simulation_time(time: Res<Time>, mut clock: ResMut<SimulationClock>) {
    clock.time += time.delta_secs() * TIME_SCALE;
}

// ============================================================================
// CAMERA & VIEW SETUP
// ============================================================================

// creates the camera for visualization
fn setup(mut commands: Commands) {
    // Spawn a simple 2D camera that supports panning and zooming via PanCam.
    commands.spawn((Camera2d, PanCam::default()));
    // commands.spawn((Camera2d));
}

// set the camera position and zoom to fit the network
fn fit_camera_to_network(
    mut commands: Commands,
    network: Option<Res<NetworkData>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    // return if no network exists
    let Some(network) = network else {
        return;
    };

    // return if the network is empty
    if network.node_positions.is_empty() {
        return;
    }

    // get the window to calc the size of the window
    let Some(window) = window_query.iter().next() else {
        return;
    };

    // calc the bounding box of all nodes
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

    // calc the network width, height and center
    let width = (max_x - min_x).max(f32::EPSILON);
    let height = (max_y - min_y).max(f32::EPSILON);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;

    // get the window dimensions
    let window_width = window.width().max(1.0);
    let window_height = window.height().max(1.0);

    // calc the scale factor in x- and y-direction
    let scale_x = width / window_width;
    let scale_y = height / window_height;

    // add some margin and use the bigger scale factor
    let margin = 1.1;
    let mut scale = scale_x.max(scale_y) * margin;
    if !scale.is_finite() || scale <= 0.0 {
        scale = 1.0;
    }

    // set the view settings
    commands.insert_resource(ViewSettings {
        center: Vec2::new(center_x, center_y),
        scale,
    });
}

// ============================================================================
// UI SETUP & UPDATE
// ============================================================================

// creates a simple ui text in the top-right corner showing simulation time and fps
fn setup_ui(mut commands: Commands) {
    commands.spawn((
        UiNode {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..Default::default()
        },
        Text::new("Sim Time: 00:00  FPS:     "),
        TextFont {
            font_size: 18.0,
            ..Default::default()
        },
        TextColor(Color::srgb(1.0, 1.0, 1.0)),
        TimeFpsText,
    ));
}

// updates the ui text every frame with the current simulation time and an approximate fps value
fn update_time_and_fps(
    mut frame_counter: Local<u32>,
    time: Res<Time>,
    clock: Res<SimulationClock>,
    mut query: Query<&mut Text, With<TimeFpsText>>,
) {
    *frame_counter += 1;
    if *frame_counter % TIME_FPS_UPDATE_EVERY_N_FRAMES != 0 {
        return;
    }

    let sim_time = clock.time;
    let delta = time.delta_secs();
    let fps = if delta > 0.0 {
        (1.0 / delta).round() as i32
    } else {
        0
    };

    // the simulation time is defined in seconds since 0:00
    let total_seconds = sim_time.max(0.0) as i32;
    let hours = (total_seconds / 3600) % 24;
    let minutes = (total_seconds / 60) % 60;

    let content = format!("Sim Time: {:02}:{:02}  FPS: {:>4}", hours, minutes, fps);

    for mut text in &mut query {
        text.0.clear();
        text.0.push_str(&content);
    }
}

// ============================================================================
// RENDERING (Network & Vehicles)
// ============================================================================

fn draw_network(mut gizmos: Gizmos, network: Res<NetworkData>, view: Option<Res<ViewSettings>>) {
    // use current view settings (if not defined use default values as fallback)
    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

    // draw the links
    for (_link_id, (from_id, to_id)) in &network.link_endpoints {
        if let (Some(from), Some(to)) = (
            network.node_positions.get(from_id),
            network.node_positions.get(to_id),
        ) {
            gizmos.line_2d(
                (*from - center) / scale,
                (*to - center) / scale,
                Color::srgb(1.0, 1.0, 1.0),
            );
        }
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
    // use current view settings (if not defined use default values as fallback)
    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

    let mut waiting_stacks: HashMap<String, u32> = HashMap::new();

    // get the current simulation time.
    let sim_time = clock.time;

    // loop over all vehicles and draw their current position.
    for (vehicle_id, trips_for_vehicle) in trips.per_vehicle.iter() {
        if trips_for_vehicle.is_empty() {
            continue;
        }

        // get the max vehicle speed
        let vehicle_v_max = vehicles
            .vehicles
            .get(vehicle_id)
            .map(|v| v.maximum_velocity)
            .unwrap_or(f32::INFINITY);

        struct VehiclePosition {
            world: Vec2,
            waiting_node: Option<String>,
        }

        let mut position_to_draw: Option<VehiclePosition> = None;
        struct ScheduledLink {
            from_pos: Vec2,
            to_pos: Vec2,
            depart_time: f32,
            arrival_time: f32,
            to_node_id: String,
        }

        'trips: for trip in trips_for_vehicle {
            if trip.links.is_empty() {
                continue;
            }

            let mut schedule: Vec<ScheduledLink> = Vec::with_capacity(trip.links.len());
            let mut prev_arrival_time_schedule: Option<f32> = None;

            for traversed_link in &trip.links {
                // Get link endpoints from the network
                let (from_id, to_id) = match network.link_endpoints.get(&traversed_link.link_id) {
                    Some(v) => v.clone(),
                    None => continue,
                };

                // get the positions of the from and to nodes.
                let (from_pos, to_pos) = match (
                    network.node_positions.get(&from_id),
                    network.node_positions.get(&to_id),
                ) {
                    (Some(&from), Some(&to)) => (from, to),
                    _ => continue,
                };

                // calc the link length based on the coordinates
                let link_vector = to_pos - from_pos;
                let link_length = link_vector.length().max(f32::EPSILON);

                // get the max speed for the link
                let link_v_max = *network
                    .link_freespeed
                    .get(&traversed_link.link_id)
                    .unwrap_or(&f32::INFINITY);

                // calc the max speed based on min(link_v_max, vehicle_v_max)
                let v_eff = vehicle_v_max.min(link_v_max);
                if v_eff <= 0.0 {
                    continue;
                }

                // calc the travel time based on the speed and the length
                let travel_duration = link_length / v_eff;
                let scheduled_start = traversed_link.start_time;

                // calc the departure time for this link
                let depart_time = match prev_arrival_time_schedule {
                    Some(arrival_prev) => scheduled_start.max(arrival_prev),
                    None => scheduled_start,
                };

                let arrival_time = depart_time + travel_duration;

                schedule.push(ScheduledLink {
                    from_pos,
                    to_pos,
                    depart_time,
                    arrival_time,
                    to_node_id: to_id.clone(),
                });

                prev_arrival_time_schedule = Some(arrival_time);
            }

            if schedule.is_empty() {
                continue;
            }

            let trip_start = schedule.first().unwrap().depart_time;
            let trip_end = schedule.last().unwrap().arrival_time;
            if sim_time < trip_start || sim_time >= trip_end {
                continue;
            }

            let mut prev_arrival_time: Option<f32> = None;
            let mut prev_arrival_pos: Option<Vec2> = None;
            let mut prev_arrival_node_id: Option<String> = None;

            for entry in &schedule {
                if let (Some(arrival_prev), Some(wait_pos)) = (prev_arrival_time, prev_arrival_pos)
                {
                    if sim_time >= arrival_prev && sim_time < entry.depart_time {
                        position_to_draw = Some(VehiclePosition {
                            world: wait_pos,
                            waiting_node: prev_arrival_node_id.clone(),
                        });
                        break 'trips;
                    }
                }

                if sim_time >= entry.depart_time && sim_time < entry.arrival_time {
                    let travel_duration =
                        (entry.arrival_time - entry.depart_time).max(f32::EPSILON);
                    let progress =
                        ((sim_time - entry.depart_time) / travel_duration).clamp(0.0, 1.0);
                    let link_vector = entry.to_pos - entry.from_pos;
                    let position = entry.from_pos + link_vector * progress;
                    position_to_draw = Some(VehiclePosition {
                        world: position,
                        waiting_node: None,
                    });
                    break 'trips;
                }

                prev_arrival_time = Some(entry.arrival_time);
                prev_arrival_pos = Some(entry.to_pos);
                prev_arrival_node_id = Some(entry.to_node_id.clone());
            }
        }

        if let Some(position_info) = position_to_draw {
            let mut position_view = (position_info.world - center) / scale;
            if let Some(node_id) = &position_info.waiting_node {
                let stack_index = waiting_stacks.entry(node_id.clone()).or_insert(0);
                position_view += Vec2::new(0.0, WAIT_STACK_OFFSET * (*stack_index as f32));
                *stack_index += 1;
            }
            gizmos.circle_2d(position_view, 4.0, Color::srgb(0.0, 1.0, 0.0));
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // create shared trips builder that can be accessed from both thread and Bevy systems
    let shared_builder = SharedTripsBuilder {
        builder: Arc::new(Mutex::new(TripsBuilder::new())),
    };

    // start the event reading thread which reads the events file
    // and processes them in real-time
    let events_channel = start_events_thread(shared_builder.clone());

    // create initial empty trips
    let trips = AllTrips {
        per_vehicle: HashMap::new(),
        first_start: 0.0,
    };

    let sim_clock = SimulationClock { time: 0.0 };

    // init bevy app
    App::new()
        .insert_resource(trips)
        .insert_resource(sim_clock)
        .insert_resource(events_channel)
        .insert_resource(shared_builder)
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "MATSim Rust OTF Viz".into(),
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
                setup_ui,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                simulation_time,
                receive_events_from_channel,
                update_trips_from_builder,
                draw_network,
                draw_vehicles,
                update_time_and_fps,
            ),
        )
        .run();
}
