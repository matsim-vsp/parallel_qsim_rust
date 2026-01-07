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
use std::mem;
use std::ops::Add;
use std::rc::Rc;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
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
// Defines the vertical offset between stacked waiting vehicles at a node
const WAIT_STACK_OFFSET: f32 = 8.0;
const FIXED_HZ: f64 = 30.0; // Ticks Per Seconds; Fixed time steps per second

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

// Tracks how many FixedUpdate ticks actually happened per real second
#[derive(Resource, Default)]
struct FixedTickStats {
    ticks_this_second: u32, // count how many FixedUpdates happened in the current second
    last_tps: u32,          // last tps value
    seconds_since_last_sample: f32, // stores how much time passed since the last_tps was updated
}

#[derive(Resource, Default)]
struct EventsProgress {
    latest_tick_time: u32, // latest tick time which was processed by the main thread
    done: bool,            // is true when all events have been processed
}

type BoxedEvent = Box<dyn EventTrait + Send>;

enum EventsTickMessage {
    Tick { time: u32, events: Vec<BoxedEvent> },
    Done,
}

// Resource that contains the receiver side of the event channel.
#[derive(Resource)]
struct EventsChannel {
    receiver: Mutex<mpsc::Receiver<EventsTickMessage>>,
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

// Resource that holds the TripsBuilder and EventsPublisher for the main thread
struct TripsBuilderResource {
    builder: Rc<RefCell<TripsBuilder>>,
    publisher: EventsPublisher,
}

// ============================================================================
// TRIPS BUILDER
// ============================================================================

#[derive(Default)]
struct TripsBuilder {
    // stores the current link of a vehicle
    // key -> vehicle ID; value -> (link ID, start time)
    current_link_per_vehicle: HashMap<String, (String, f32)>,
    // stores the currently active trip per vehicle (Vec of TraversedLink)
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

    // Check the event type from the incoming event and use the correct handler
    fn handle_event(&mut self, event: &dyn EventTrait) {
        // Try to downcast to LinkEnterEvent
        if let Some(enter) = event.as_any().downcast_ref::<LinkEnterEvent>() {
            self.handle_link_enter(enter);
        // Try to downcast to LinkLeaveEvent
        } else if let Some(leave) = event.as_any().downcast_ref::<LinkLeaveEvent>() {
            self.handle_link_leave(leave);
        // Try to downcast to PersonEntersVehicleEvent
        } else if let Some(enter) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            self.handle_person_enters(enter);
        // Try to downcast to PersonLeavesVehicleEvent
        } else if let Some(leave) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            self.handle_person_leaves(leave);
        }
    }

    // When a vehicle enters a link, the link ID and start time are stored in current_link_per_vehicle
    fn handle_link_enter(&mut self, event: &LinkEnterEvent) {
        // Extract link and vehicle IDs as strings
        let link_id = event.link.external().to_string();
        let vehicle_id = event.vehicle.external().to_string();
        // Store: vehicle_id -> (link_id, start_time)
        self.current_link_per_vehicle
            .insert(vehicle_id, (link_id, event.time as f32));
    }

    // When a vehicle leaves a link, a TraversedLink is created and added to the current trip
    fn handle_link_leave(&mut self, event: &LinkLeaveEvent) {
        // Extract link and vehicle IDs
        let link_id = event.link.external().to_string();
        let vehicle_id = event.vehicle.external().to_string();
        // Get the start time and link id when the vehicle entered the link
        if let Some((entered_link, start_time)) = self.current_link_per_vehicle.remove(&vehicle_id)
        {
            let end_time = event.time as f32;
            // Check if the link id is the same and the start time is earlier than end time
            if entered_link == link_id && end_time >= start_time {
                // Update earliest start time if this is earlier
                if start_time < self.first_start {
                    self.first_start = start_time;
                }
                // Add the link only if there is an active trip
                if let Some(current_trip) = self.current_trip_per_vehicle.get_mut(&vehicle_id) {
                    current_trip.push(TraversedLink {
                        link_id,
                        start_time,
                    });
                }
            }
        }
    }

    // Start a new trip when a person enters a vehicle
    fn handle_person_enters(&mut self, event: &PersonEntersVehicleEvent) {
        let vehicle_id = event.vehicle.external().to_string();
        // Create an empty Vec for collecting TraversedLinks
        self.current_trip_per_vehicle
            .entry(vehicle_id)
            .or_insert_with(Vec::new);
    }

    // Finish the current trip when a person leaves a vehicle
    fn handle_person_leaves(&mut self, event: &PersonLeavesVehicleEvent) {
        let vehicle_id = event.vehicle.external().to_string();
        // Remove the active trip from current_trip_per_vehicle
        if let Some(trip_links) = self.current_trip_per_vehicle.remove(&vehicle_id) {
            // Save only trips that are not empty
            if !trip_links.is_empty() {
                // Move the trip to finished trips (per_vehicle)
                self.per_vehicle
                    .entry(vehicle_id)
                    .or_default()
                    .push(Trip { links: trip_links });
            }
        }
    }

    // Build AllTrips object from current builder state (includes both finished and ongoing trips)
    fn build_all_trips(&self) -> AllTrips {
        // Create a new HashMap to collect all trips
        // key -> vehicle ID; value -> Vec of Trips
        let mut per_vehicle: HashMap<String, Vec<Trip>> = HashMap::new();

        // Copy all finished trips from per_vehicle
        for (veh_id, trips) in &self.per_vehicle {
            per_vehicle.insert(veh_id.clone(), trips.clone());
        }

        // Store also the current trips that are still ongoing
        for (veh_id, current_links) in &self.current_trip_per_vehicle {
            // Only include if the trip has at least one link
            if !current_links.is_empty() {
                // Add the ongoing trip to the vehicle's trips
                per_vehicle.entry(veh_id.clone()).or_default().push(Trip {
                    links: current_links.clone(),
                });
            }
        }

        // Sort all trips per vehicle
        for trips in per_vehicle.values_mut() {
            // Sort links within each trip by start time
            for trip in trips.iter_mut() {
                trip.links.sort_by(|a, b| {
                    a.start_time
                        .partial_cmp(&b.start_time)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            // Sort trips by their start time
            trips.sort_by(|a, b| {
                let a_start = a.links.first().map(|t| t.start_time).unwrap_or(f32::MAX);
                let b_start = b.links.first().map(|t| t.start_time).unwrap_or(f32::MAX);
                a_start
                    .partial_cmp(&b_start)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        // Set first_start to 0 if no events were processed yet
        let mut first_start = self.first_start;
        if first_start == f32::MAX {
            first_start = 0.0;
        }

        // Return the AllTrips object
        AllTrips {
            per_vehicle,
            first_start,
        }
    }
}

// ============================================================================
// EVENT PROCESSING & THREADING
// ============================================================================

// Register a callback that forwards all published events to the TripsBuilder
fn register_trips_listener(builder: Rc<RefCell<TripsBuilder>>, publisher: &mut EventsPublisher) {
    // Clone the builder to use it inside the closure
    let builder_for_events = builder.clone();

    // Register a closure that will be called for every published event
    publisher.on_any(move |event| {
        // Get mutable access to the builder and handle the event
        builder_for_events.borrow_mut().handle_event(event);
    });
}

// Convert proto events to internal event objects and publish them
fn process_events(time: u32, mut events: Vec<MyEvent>) -> Vec<BoxedEvent> {
    let mut internal_events: Vec<BoxedEvent> = Vec::with_capacity(events.len());

    for proto_event in events.iter_mut() {
        if !proto_event.attributes.contains_key("type") {
            proto_event.attributes.insert(
                "type".to_string(),
                AttributeValue::from(proto_event.r#type.as_str()),
            );
        }

        let type_ = proto_event.attributes["type"].as_string();
        let internal_event: BoxedEvent = match type_.as_str() {
            GeneralEvent::TYPE => Box::new(GeneralEvent::from_proto_event(proto_event, time)),
            ActivityStartEvent::TYPE => {
                Box::new(ActivityStartEvent::from_proto_event(proto_event, time))
            }
            ActivityEndEvent::TYPE => {
                Box::new(ActivityEndEvent::from_proto_event(proto_event, time))
            }
            LinkEnterEvent::TYPE => Box::new(LinkEnterEvent::from_proto_event(proto_event, time)),
            LinkLeaveEvent::TYPE => Box::new(LinkLeaveEvent::from_proto_event(proto_event, time)),
            PersonEntersVehicleEvent::TYPE => Box::new(PersonEntersVehicleEvent::from_proto_event(
                proto_event,
                time,
            )),
            PersonLeavesVehicleEvent::TYPE => Box::new(PersonLeavesVehicleEvent::from_proto_event(
                proto_event,
                time,
            )),
            PersonDepartureEvent::TYPE => {
                Box::new(PersonDepartureEvent::from_proto_event(proto_event, time))
            }
            PersonArrivalEvent::TYPE => {
                Box::new(PersonArrivalEvent::from_proto_event(proto_event, time))
            }
            TeleportationArrivalEvent::TYPE => Box::new(
                TeleportationArrivalEvent::from_proto_event(proto_event, time),
            ),
            PtTeleportationArrivalEvent::TYPE => Box::new(
                PtTeleportationArrivalEvent::from_proto_event(proto_event, time),
            ),
            _ => panic!("Unknown event type: {:?}", type_),
        };

        internal_events.push(internal_event);
    }

    internal_events
}

// Start a background thread that reads events from the file and sends them through a channel
fn start_events_thread() -> (EventsChannel, JoinHandle<()>) {
    const EVENT_TIME_STEP: u32 = 1;
    const CHANNEL_BUFFER_TICKS: usize = 256;

    let (tx, rx) = mpsc::sync_channel::<EventsTickMessage>(CHANNEL_BUFFER_TICKS);

    // Create a background thread that reads the events file and sends fixed time-step ticks.
    // - For each simulation second t, one Tick (time: t, events: ...) will be sent to the main thread.
    // - If there are no events, an empty events Vec is sent.
    // - When the file is fully read, a Done message is sent.
    let handle = thread::spawn(move || {
        // println!("Before; Current time = {:?}", std::time::SystemTime::now());
        // thread::sleep(Duration::from_secs(10));
        // println!("After; Current time = {:?}", std::time::SystemTime::now());
        // Create a reader to iterate through the events file.
        let reader = ProtoEventsReader::new(Cursor::new(EVENTS_FILE));

        // stores the nex time to send
        let mut next_time_to_send: Option<u32> = None;

        // stores the current simulation time
        let mut buffered_time: Option<u32> = None;
        // Store the events which will be sent the next time
        let mut buffered_events: Vec<MyEvent> = Vec::new();

        let mut event_tick_count: usize = 0;

        // flush_time_slot is a closure that takes a time and events, processes them, and sends them through the channel.
        let mut flush_time_slot = |time: u32, events_at_time: Vec<MyEvent>| {
            let cursor = next_time_to_send.get_or_insert(time);

            // Sends empty ticks until we reach the desired time slot.
            // cursor defines the next time to send and time is the current time slot to flush
            // if the cursor is less than time, we need to send empty ticks until we reach time
            while *cursor < time {
                if tx
                    .send(EventsTickMessage::Tick {
                        time: *cursor,
                        events: Vec::new(),
                    })
                    .is_err()
                {
                    return false;
                }
                *cursor = cursor.add(EVENT_TIME_STEP);
            }

            event_tick_count += 1;

            // convert all proto events to internal events and send them to the main thread
            let internal_events = process_events(time, events_at_time);
            if tx
                .send(EventsTickMessage::Tick {
                    time,
                    events: internal_events,
                })
                .is_err()
            {
                return false;
            }

            *cursor = time.add(EVENT_TIME_STEP);
            // thread::sleep(Duration::from_millis(20));
            // println!("currnet time = {}", time);

            if event_tick_count > 10 {
                println!("Lets wait a bit...");
                thread::sleep(Duration::from_millis(200));
            }

            true
        };

        // Read all events from the file and send them to the main thread in fixed time steps.
        // time = sim time
        // events_at_time = all events that happen at this time
        for (time, events_at_time) in reader {
            match buffered_time {
                None => {
                    // in the first iteration we store the time and events
                    // because the viz should start from the first event time
                    buffered_time = Some(time);
                    buffered_events = events_at_time;
                }
                Some(t) if time <= t => {
                    // if the events belong to the current time slot, we append them
                    buffered_events.extend(events_at_time);
                }
                Some(t) if time > t => {
                    // if the current time slot is finished, the events are send via flush_time_slot to the main thread
                    let events_to_flush = mem::take(&mut buffered_events);

                    // send the buffered events via flush_time_slot to the main thread
                    flush_time_slot(t, events_to_flush);
                    buffered_time = Some(time);
                    buffered_events = events_at_time;
                }
                Some(t) => panic!("Events file is not sorted (time went backwards: {t} -> {time})"),
            }
        }

        // send a done message after all events have been processed
        let _ = tx.send(EventsTickMessage::Done);
    });

    // Returns the receiver
    (
        EventsChannel {
            receiver: Mutex::new(rx),
        },
        handle,
    )
}

// Reads all available messages from the events channel and sends them to the TripsBuilder
// This tracks also the time from the latest tick that was processed
fn process_events_from_channel(
    events_channel: Res<EventsChannel>,
    mut builder_resource: NonSendMut<TripsBuilderResource>,
    mut progress: ResMut<EventsProgress>,
    mut done: Local<bool>,
) {
    // return immediately if done
    if *done {
        return;
    }

    // Collect all currently available event tick messages
    let mut current_available_events: Vec<EventsTickMessage> = Vec::new();
    {
        // Lock the receiver
        let Ok(receiver) = events_channel.receiver.lock() else {
            // if its not possible (to lock) -> return ans try next time..
            return;
        };

        //get all currently available event tick messages
        loop {
            match receiver.try_recv() {
                Ok(msg) => current_available_events.push(msg),
                Err(mpsc::TryRecvError::Empty) => {
                    // if there are no more messages -> exit the loop
                    break;
                } // Nothing more available right now
                Err(mpsc::TryRecvError::Disconnected) => {
                    // if the connection is disconnected -> handle this like a done message
                    *done = true;
                    break;
                }
            }
        }
    }

    // Process events
    for msg in current_available_events {
        match msg {
            EventsTickMessage::Tick { time, events } => {
                // store the latest tick time
                progress.latest_tick_time = progress.latest_tick_time.max(time);

                // Publish each event
                for event in &events {
                    builder_resource.publisher.publish_event(event.as_ref());
                }
            }
            EventsTickMessage::Done => {
                // if all events are processed (Done) -> set done = true
                progress.done = true;
                *done = true;
                break;
            }
        }
    }
}

// Updates AllTrips from the builder
fn update_trips_from_builder(
    mut clock_initialized: Local<bool>,
    builder_resource: NonSend<TripsBuilderResource>,
    mut trips: ResMut<AllTrips>,
    mut clock: ResMut<SimulationClock>,
) {
    // Build AllTrips from the current builder state
    let new_trips = builder_resource.builder.borrow().build_all_trips();

    // On first update: Initialize the simulation clock to start at the first event time
    if !*clock_initialized && new_trips.first_start > 0.0 && new_trips.first_start != f32::MAX {
        // set the clock to the time of the first event
        clock.time = new_trips.first_start;
        *clock_initialized = true;
    }

    // Overwrite the AllTrips resource with the new data
    *trips = new_trips;
}

// ============================================================================
// DATA LOADING (Network & Vehicles)
// ============================================================================

// Bevy startup system: Read and parse the network protobuf file and create NetworkData resource
fn read_and_parse_network(mut commands: Commands) {
    // Decode the protobuf network from the embedded bytes
    let wire: wire_network::Network =
        wire_network::Network::decode(NETWORK_FILE).expect("Failed to decode network protobuf");
    let mut network = NetworkData::default();

    // Loop through all nodes in the protobuf file
    for wn in &wire.nodes {
        // Extract node ID and coordinates
        let id = wn.id.clone();
        let x: f32 = wn.x as f32;
        let y: f32 = wn.y as f32;

        // Store the node position (ID -> Vec2) in the HashMap
        network.node_positions.insert(id, Vec2::new(x, y));
    }

    // Loop through all links in the protobuf file
    for wl in &wire.links {
        // Extract link ID, from/to node IDs, and freespeed
        let id = wl.id.clone();
        let from_id = wl.from.clone();
        let to_id = wl.to.clone();
        let freespeed: f32 = wl.freespeed;

        // Store the link endpoints (link_id -> (from_node, to_node))
        network.link_endpoints.insert(id, (from_id, to_id));
        // Store the link freespeed (link_id -> max_speed)
        network.link_freespeed.insert(wl.id.clone(), freespeed);
    }

    // Insert the NetworkData as a Bevy resource
    commands.insert_resource(network);
}

// Bevy startup system: Read and parse the vehicles protobuf file and create VehiclesData resource
fn read_and_parse_vehicles(mut commands: Commands) {
    // Decode the protobuf vehicles container from the embedded bytes
    let wire: wire_vehicles::VehiclesContainer =
        wire_vehicles::VehiclesContainer::decode(VEHICLES_FILE)
            .expect("Failed to decode vehicles protobuf");

    let mut vehicles: HashMap<String, Vehicle> = HashMap::new();

    // Loop through all vehicles in the protobuf file
    for wv in &wire.vehicles {
        // Extract vehicle ID and maximum velocity
        let id = wv.id.to_string();
        let maximum_velocity = wv.max_v;

        // Store the vehicle (ID -> Vehicle) in the HashMap
        vehicles.insert(id.clone(), Vehicle { maximum_velocity });
    }

    // Insert the VehiclesData as a Bevy resource
    commands.insert_resource(VehiclesData { vehicles });
}

// ============================================================================
// SIMULATION TIME
// ============================================================================

// calculates the simulation time based on real time and event progress
// the next time is the minimum of the next simulation time and latest processed tick time + 1.0
// this runs in every update to make the visualization smooth
fn simulation_time(
    real: Res<Time<Real>>,
    mut clock: ResMut<SimulationClock>,
    progress: Res<EventsProgress>,
) {
    // this is the next simulation time based on the real time and time scale
    let next = clock.time + real.delta_secs() * TIME_SCALE;

    // latest_tick_time is the latest time where the main thread received events
    // -> the next simulation time should not be bigger than latest_tick_time + 1.0
    let max_time = (progress.latest_tick_time as f32) + 1.0;

    // calculates the minimum of next and max_time
    clock.time = next.min(max_time);
}

// ============================================================================
// CAMERA & VIEW SETUP
// ============================================================================

// Create the 2D camera for visualization
fn setup(mut commands: Commands) {
    // Spawn a 2D camera with PanCam plugin (enables mouse panning and zooming)
    commands.spawn((Camera2d, PanCam::default()));
}

// Calculate camera position and zoom to fit the entire network
fn fit_camera_to_network(
    mut commands: Commands,
    network: Option<Res<NetworkData>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    // Early return if no network resource exists yet
    let Some(network) = network else {
        return;
    };

    // Early return if the network has no nodes
    if network.node_positions.is_empty() {
        return;
    }

    // Get the primary window to determine viewport size
    let Some(window) = window_query.iter().next() else {
        return;
    };

    // Calculate the bounding box of all nodes (find min/max x and y)
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    // Loop through all node positions to find the bounds
    for position in network.node_positions.values() {
        min_x = min_x.min(position.x);
        max_x = max_x.max(position.x);
        min_y = min_y.min(position.y);
        max_y = max_y.max(position.y);
    }

    // Safety check: ensure all bounds are valid numbers
    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return;
    }

    // Calculate network dimensions and center point
    let width = (max_x - min_x).max(f32::EPSILON); // Avoid division by zero
    let height = (max_y - min_y).max(f32::EPSILON);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;

    // Get window dimensions (with minimum value to avoid division by zero)
    let window_width = window.width().max(1.0);
    let window_height = window.height().max(1.0);

    // Calculate scale factors for x and y directions
    // scale = world_units / screen_pixels
    let scale_x = width / window_width;
    let scale_y = height / window_height;

    // Use the larger scale factor (ensures entire network fits) and add 10% margin
    let margin = 1.1;
    let mut scale = scale_x.max(scale_y) * margin;
    // Safety check: ensure scale is valid and positive
    if !scale.is_finite() || scale <= 0.0 {
        scale = 1.0;
    }

    // Create and insert ViewSettings resource with calculated center and scale
    commands.insert_resource(ViewSettings {
        center: Vec2::new(center_x, center_y),
        scale,
    });
}

// ============================================================================
// UI SETUP & UPDATE
// ============================================================================

// Create UI text element in the top-right corner showing simulation time and FPS
fn setup_ui(mut commands: Commands) {
    // Create a text UI entity for displaying time, FPS and TPS
    commands.spawn((
        // Position the text absolutely in the top-right corner
        UiNode {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),   // 10 pixels from top
            right: Val::Px(10.0), // 10 pixels from right
            ..Default::default()
        },
        // Initial placeholder text
        Text::new("Sim Time: 00:00  FPS:     "),
        TextFont {
            font_size: 18.0,
            ..Default::default()
        },
        TextColor(Color::srgb(1.0, 1.0, 1.0)),
        TimeFpsText,
    ));
}

// Runs in FixedUpdate: count how many fixed ticks happened to display the actual TPS
fn count_fixed_ticks(mut stats: ResMut<FixedTickStats>) {
    stats.ticks_this_second += 1;
}

// Runs every frame in Update: measure real time and snapshot the tick count once per second
fn sample_tps(mut stats: ResMut<FixedTickStats>, real: Res<Time<Real>>) {
    stats.seconds_since_last_sample += real.delta_secs();

    if stats.seconds_since_last_sample >= 1.0 {
        stats.last_tps = stats.ticks_this_second;
        stats.ticks_this_second = 0;
        stats.seconds_since_last_sample -= 1.0;
    }
}

// Updates the UI text every frame with the current simulation time, FPS, and the measured fixed-update rate.
fn update_time_and_fps(
    time: Res<Time<Virtual>>,    // Frame time (runs in `Update`)
    clock: Res<SimulationClock>, // Simulation clock (drives vehicle positions)
    stats: Res<FixedTickStats>,  // Measured FixedUpdate ticks per real second
    mut query: Query<&mut Text, With<TimeFpsText>>,
) {
    // Read the current simulation time in seconds.
    let sim_time = clock.time;

    // Estimate FPS from the last frame duration.
    let frame_delta = time.delta_secs();
    let fps = if frame_delta > 0.0 {
        (1.0 / frame_delta).round() as i32
    } else {
        0
    };

    // latest tps value
    let tps = stats.last_tps as i32;

    // Format simulation time (HH:MM)
    let total_seconds = sim_time.max(0.0) as i32;
    let hours = (total_seconds / 3600) % 24;
    let minutes = (total_seconds / 60) % 60;

    // Build the UI string
    let content = format!(
        "Sim Time: {:02}:{:02}  FPS: {:>4}  TPS: {:>2}",
        hours, minutes, fps, tps
    );

    // Update the text component
    for mut text in &mut query {
        text.0.clear();
        text.0.push_str(&content);
    }
}

// ============================================================================
// RENDERING (Network & Vehicles)
// ============================================================================

// Render the network
fn draw_network(mut gizmos: Gizmos, network: Res<NetworkData>, view: Option<Res<ViewSettings>>) {
    // Get view settings (center and scale) or use defaults if not available yet
    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON)) // Avoid division by zero
    } else {
        (Vec2::ZERO, 1.0) // Default: no offset, no scaling
    };

    // Loop through all links in the network
    for (_link_id, (from_id, to_id)) in &network.link_endpoints {
        // Try to get the world positions for both nodes
        if let (Some(from), Some(to)) = (
            network.node_positions.get(from_id),
            network.node_positions.get(to_id),
        ) {
            // Transform world coordinates to view coordinates: (world - center) / scale
            // Then draw a white line between the two nodes
            gizmos.line_2d(
                (*from - center) / scale,
                (*to - center) / scale,
                Color::srgb(1.0, 1.0, 1.0), // White color
            );
        }
    }
}

// Render all vehicles
fn draw_vehicles(
    mut gizmos: Gizmos,
    trips: Res<AllTrips>,
    network: Res<NetworkData>,
    vehicles: Res<VehiclesData>,
    view: Option<Res<ViewSettings>>,
    clock: Res<SimulationClock>,
) {
    // Get view settings (center and scale) or use defaults if not available yet
    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON)) // Avoid division by zero
    } else {
        (Vec2::ZERO, 1.0) // Default: no offset, no scaling
    };

    // Track how many vehicles are waiting at each node
    let mut waiting_stacks: HashMap<String, u32> = HashMap::new();

    // Get current simulation time
    let sim_time = clock.time;

    // Loop through all vehicles and calculate their current position
    for (vehicle_id, trips_for_vehicle) in trips.per_vehicle.iter() {
        // Skip vehicles with no trips
        if trips_for_vehicle.is_empty() {
            continue;
        }

        // Get the vehicle's maximum speed
        let vehicle_v_max = vehicles
            .vehicles
            .get(vehicle_id)
            .map(|v| v.maximum_velocity)
            .unwrap_or(f32::INFINITY);

        // Struct to hold the calculated position and waiting status
        struct VehiclePosition {
            world: Vec2,                  // position
            waiting_node: Option<String>, // Node ID if the vehicle is waiting at a node
        }

        // Struct to hold scheduled link traversal with calculated times
        struct ScheduledLink {
            from_pos: Vec2,
            to_pos: Vec2,
            depart_time: f32,
            arrival_time: f32,
            to_node_id: String,
        }

        // Find vehicle position by searching through all trips
        let position_to_draw = trips_for_vehicle.iter().find_map(|trip| {
            // Skip empty trips
            if trip.links.is_empty() {
                return None;
            }

            // Build a schedule with calculated departure and arrival times
            let mut schedule: Vec<ScheduledLink> = Vec::with_capacity(trip.links.len());
            let mut prev_arrival_time_schedule: Option<f32> = None;

            // Process each link in the trip
            for traversed_link in &trip.links {
                // Get link endpoints from network
                let (from_id, to_id) = match network.link_endpoints.get(&traversed_link.link_id) {
                    Some(v) => v.clone(),
                    None => continue, // Skip invalid link
                };

                // Get node positions
                let (from_pos, to_pos) = match (
                    network.node_positions.get(&from_id),
                    network.node_positions.get(&to_id),
                ) {
                    (Some(&from), Some(&to)) => (from, to),
                    _ => continue,
                };

                // Calculate link length
                let link_vector = to_pos - from_pos;
                let link_length = link_vector.length().max(f32::EPSILON);

                // Get link's maximum speed
                let link_v_max = *network
                    .link_freespeed
                    .get(&traversed_link.link_id)
                    .unwrap_or(&f32::INFINITY);

                // effective speed (minimum of vehicle and link speed)
                let v_eff = vehicle_v_max.min(link_v_max);

                if v_eff <= 0.0 {
                    continue;
                }

                // Calculate travel duration: distance / speed
                let travel_duration = link_length / v_eff;
                let scheduled_start = traversed_link.start_time;

                // Departure time = max(scheduled_start, previous_arrival)
                // This handles waiting at nodes between links
                let depart_time = match prev_arrival_time_schedule {
                    Some(arrival_prev) => scheduled_start.max(arrival_prev),
                    None => scheduled_start, // First link uses scheduled start
                };

                // Arrival time = departure + travel duration
                let arrival_time = depart_time + travel_duration;

                // Add this link to the schedule
                schedule.push(ScheduledLink {
                    from_pos,
                    to_pos,
                    depart_time,
                    arrival_time,
                    to_node_id: to_id.clone(),
                });

                prev_arrival_time_schedule = Some(arrival_time);
            }

            // Skip if schedule is empty (all links were invalid)
            if schedule.is_empty() {
                return None;
            }

            // Get trip time range
            let trip_start = schedule.first().unwrap().depart_time;
            let trip_end = schedule.last().unwrap().arrival_time;

            // Skip this trip if current time is outside its time range
            if sim_time < trip_start || sim_time >= trip_end {
                return None;
            }

            // Track previous arrival for detecting waiting periods
            let mut prev_arrival_time: Option<f32> = None;
            let mut prev_arrival_pos: Option<Vec2> = None;
            let mut prev_arrival_node_id: Option<String> = None;

            // Find where the vehicle is at current sim_time
            for entry in &schedule {
                // Check if vehicle is waiting at a node
                if let (Some(arrival_prev), Some(wait_pos)) = (prev_arrival_time, prev_arrival_pos)
                {
                    if sim_time >= arrival_prev && sim_time < entry.depart_time {
                        // Vehicle is waiting at the node
                        return Some(VehiclePosition {
                            world: wait_pos,
                            waiting_node: prev_arrival_node_id.clone(),
                        });
                    }
                }

                // Check if vehicle is currently traversing this link
                if sim_time >= entry.depart_time && sim_time < entry.arrival_time {
                    // Calculate progress along the link (between 0.0 and 1.0)
                    let travel_duration =
                        (entry.arrival_time - entry.depart_time).max(f32::EPSILON);
                    let progress =
                        ((sim_time - entry.depart_time) / travel_duration).clamp(0.0, 1.0);
                    // Interpolate position along the link
                    let link_vector = entry.to_pos - entry.from_pos;
                    let position = entry.from_pos + link_vector * progress;
                    return Some(VehiclePosition {
                        world: position,
                        waiting_node: None,
                    });
                }

                // Update tracking variables for next iteration
                prev_arrival_time = Some(entry.arrival_time);
                prev_arrival_pos = Some(entry.to_pos);
                prev_arrival_node_id = Some(entry.to_node_id.clone());
            }

            None
        });

        // Draw the vehicle if a position was found
        if let Some(position_info) = position_to_draw {
            // Transform world coordinates to view coordinates
            let mut position_view = (position_info.world - center) / scale;
            // If vehicle is waiting at a node, stack it vertically with other waiting vehicles
            if let Some(node_id) = &position_info.waiting_node {
                // Get the current stack index for this node (or 0 if first)
                let stack_index = waiting_stacks.entry(node_id.clone()).or_insert(0);
                // Offset position vertically based on stack index
                position_view += Vec2::new(0.0, WAIT_STACK_OFFSET * (*stack_index as f32));
                // Increment stack counter for next vehicle at this node
                *stack_index += 1;
            }
            // Draw a green circle at the calculated position
            gizmos.circle_2d(position_view, 4.0, Color::srgb(0.0, 1.0, 0.0));
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // Start the background thred that reads events from file and sends them to the channel
    let (events_channel, handle) = start_events_thread();
    // Create the trips builder
    let builder = Rc::new(RefCell::new(TripsBuilder::new()));
    // Create the event publisher
    let mut publisher = EventsPublisher::new();
    // Register the builder as a listener
    register_trips_listener(builder.clone(), &mut publisher);

    // Bundle builder and publisher into a single resource
    let builder_resource = TripsBuilderResource { builder, publisher };

    // Create initial empty AllTrips resource
    let trips = AllTrips {
        per_vehicle: HashMap::new(),
        first_start: 0.0,
    };

    // Create simulation clock starting at time 0
    let sim_clock = SimulationClock { time: 0.0 };

    // Initialize and run the Bevy application
    App::new()
        // Insert resources that will be available to all systems
        .insert_resource(Time::<Fixed>::from_hz(FIXED_HZ))
        .insert_resource(trips)
        .insert_resource(sim_clock)
        .insert_resource(FixedTickStats::default())
        .insert_resource(EventsProgress::default())
        .insert_resource(events_channel)
        .insert_non_send_resource(builder_resource)
        // Add Bevy plugins
        .add_plugins((
            // Default Bevy plugins with custom window settings
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "MATSim Rust OTF Viz".into(), // Window title
                    resolution: (1200, 800).into(),      // Window size
                    resizable: true,                     // Allow resizing
                    ..default()
                }),
                ..default()
            }),
            // PanCam plugin for camera panning and zooming
            PanCamPlugin::default(),
        ))
        // Add startup systems
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
        // Add update systems
        .add_systems(
            FixedUpdate,
            (
                count_fixed_ticks,
                process_events_from_channel,
                update_trips_from_builder,
            ),
        )
        .add_systems(
            Update,
            (
                simulation_time,
                sample_tps,
                draw_network,
                draw_vehicles,
                update_time_and_fps,
            )
                .chain(),
        )
        // Start the application
        .run();
    handle.join().unwrap();
}
