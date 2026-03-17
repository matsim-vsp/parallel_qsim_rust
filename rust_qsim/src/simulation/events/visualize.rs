use crate::simulation::events::{
    EventHandlerRegisterFn, EventsManager, LinkEnterEvent, LinkLeaveEvent, PersonDepartureEvent,
    PersonEntersVehicleEvent, PersonLeavesVehicleEvent,
};
use crate::simulation::framework_events::{
    MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn,
};
use crate::simulation::network::Network;
use crate::simulation::vehicles::garage::Garage;
use bevy::prelude::*;
use bevy::ui::Node as UiNode;
use bevy::window::PrimaryWindow;
use bevy_pancam::{PanCam, PanCamPlugin};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// This controls how fast simulation time should move compared to real time.
// Example:
// - 1.0 means: 1 simulation second takes 1 real second.
// - 50.0 means: 50 simulation seconds take 1 real second.
// We keep it as one constant so speed can be tuned in one place.
const REALTIME_SCALE: f64 = 50.0;

// Message format that travels from simulation thread to UI thread.
// Data flow in simple words:
// 1) simulation creates one of these messages,
// 2) message is sent through channel,
// 3) UI reads it and updates clock/trips/drawing.
#[derive(Debug, Clone)]
pub enum VisualizeEventMessage {
    // Sent before each simulation step.
    // This is the main heartbeat for simulation time in the UI.
    BeforeSimStep {
        time: u32,
    },
    // Vehicle enters one network link at `time`.
    // UI uses this to grow trip path data.
    LinkEnter {
        time: u32,
        link_id: String,
        vehicle_id: String,
    },
    // Vehicle leaves one network link at `time`.
    // Useful to know when a link movement window ends.
    LinkLeave {
        time: u32,
        link_id: String,
        vehicle_id: String,
    },
    // Person boards a vehicle at `time`.
    // In this module that starts/keeps an active trip for this vehicle.
    PersonEntersVehicle {
        time: u32,
        vehicle_id: String,
    },
    // Person leaves a vehicle at `time`.
    // In this module that closes and stores the trip.
    PersonLeavesVehicle {
        time: u32,
        vehicle_id: String,
    },
    // Simulation has finished and no more events will come.
    Done,
}

#[derive(Resource, Clone)]
struct PauseControl {
    // Shared pause flag:
    // UI writes this on button click,
    // simulation loop reads it before next step.
    pause_requested: Arc<AtomicBool>,
}

#[derive(Default)]
struct SimSpeedSyncState {
    // Real-time anchor.
    real_time_reference: Option<Instant>,
    // Simulation-time anchor that belongs to same moment as above.
    simulation_time_reference: Option<u32>,
}

fn sync_simulation_speed_to_realtime(state: &Mutex<SimSpeedSyncState>, sim_time: u32) {
    // Invalid scale means we cannot do meaningful timing math.
    if REALTIME_SCALE <= 0.0 {
        return;
    }

    // If lock fails, skip syncing instead of crashing visualization.
    let Ok(mut state) = state.lock() else {
        return;
    };

    // First call initializes our reference pair:
    // "this simulation time happened at this real instant".
    if state.simulation_time_reference.is_none() || state.real_time_reference.is_none() {
        state.simulation_time_reference = Some(sim_time);
        state.real_time_reference = Some(Instant::now());
        return;
    }

    // Read reference values. If missing, syncing is skipped.
    let Some(sim_start_time) = state.simulation_time_reference else {
        return;
    };
    let Some(real_time_start) = state.real_time_reference.as_ref() else {
        return;
    };

    // Elapsed simulation seconds since reference point.
    let elapsed_sim_seconds = (sim_time - sim_start_time) as f64;

    // How many real seconds should have passed at the chosen speed.
    let target_elapsed_real_seconds = elapsed_sim_seconds / REALTIME_SCALE;

    // How many real seconds actually passed.
    let elapsed_real_seconds = real_time_start.elapsed().as_secs_f64();

    // If simulation is ahead, wait a bit.
    // If simulation is already slower, do not wait extra.
    if target_elapsed_real_seconds > elapsed_real_seconds {
        thread::sleep(Duration::from_secs_f64(
            target_elapsed_real_seconds - elapsed_real_seconds,
        ));
    }
}

pub struct VisualizeEvents;

impl VisualizeEvents {
    // Registers handlers for regular simulation events.
    // Goal: convert internal events into lightweight channel messages for the UI thread.
    pub fn register_fn(
        sender: mpsc::Sender<VisualizeEventMessage>,
        first_link_enter_seen: Arc<AtomicBool>,
    ) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            let sender_on_event = sender.clone();
            // We start real-time pacing only after first real movement.
            // Reason: some scenarios start at a late clock time and would otherwise "wait in silence".
            let first_link_enter_seen_on_event = first_link_enter_seen.clone();
            events.on_any(move |event| {
                // Map one internal event into one optional visualize message.
                let msg = if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
                    first_link_enter_seen_on_event.store(true, Ordering::Relaxed);
                    println!(
                        "[viz] LinkEnter empfangen | time={} | link={} | vehicle={}",
                        e.time,
                        e.link.external(),
                        e.vehicle.external()
                    );
                    Some(VisualizeEventMessage::LinkEnter {
                        time: e.time,
                        link_id: e.link.external().to_string(),
                        vehicle_id: e.vehicle.external().to_string(),
                    })
                } else if let Some(e) = event.as_any().downcast_ref::<PersonDepartureEvent>() {
                    println!(
                        "[viz] PersonDeparture empfangen | time={} | person={} | link={} | mode={}",
                        e.time,
                        e.person.external(),
                        e.link.external(),
                        e.leg_mode.external()
                    );
                    None
                } else if let Some(e) = event.as_any().downcast_ref::<LinkLeaveEvent>() {
                    println!(
                        "[viz] LinkLeave empfangen | time={} | link={} | vehicle={}",
                        e.time,
                        e.link.external(),
                        e.vehicle.external()
                    );
                    Some(VisualizeEventMessage::LinkLeave {
                        time: e.time,
                        link_id: e.link.external().to_string(),
                        vehicle_id: e.vehicle.external().to_string(),
                    })
                } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
                    println!(
                        "[viz] PersonEntersVehicle empfangen | time={} | person={} | vehicle={}",
                        e.time,
                        e.person.external(),
                        e.vehicle.external()
                    );
                    Some(VisualizeEventMessage::PersonEntersVehicle {
                        time: e.time,
                        vehicle_id: e.vehicle.external().to_string(),
                    })
                } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
                    println!(
                        "[viz] PersonLeavesVehicle empfangen | time={} | person={} | vehicle={}",
                        e.time,
                        e.person.external(),
                        e.vehicle.external()
                    );
                    Some(VisualizeEventMessage::PersonLeavesVehicle {
                        time: e.time,
                        vehicle_id: e.vehicle.external().to_string(),
                    })
                } else {
                    None
                };

                // Send only mapped messages.
                // If receiver is gone, we ignore the error to keep simulation robust.
                if let Some(message) = msg {
                    let _ = sender_on_event.send(message);
                }
            });

            let sender_on_finish = sender.clone();
            events.on_finish(move || {
                // Explicit end marker so UI can stop polling.
                let _ = sender_on_finish.send(VisualizeEventMessage::Done);
            });
        })
    }

    // Registers handler for simulation step events.
    // This path mainly transports time and controls pacing/pause.
    pub fn register_mobsim_fn(
        sender: mpsc::Sender<VisualizeEventMessage>,
        first_link_enter_seen: Arc<AtomicBool>,
        pause_requested: Arc<AtomicBool>,
    ) -> Box<MobsimListenerRegisterFn> {
        // Shared state for speed synchronization anchors.
        let speed_sync_state = Arc::new(Mutex::new(SimSpeedSyncState::default()));

        Box::new(move |events: &mut MobsimEventsManager| {
            let first_link_enter_seen = first_link_enter_seen.clone();
            let speed_sync_state = speed_sync_state.clone();
            let pause_requested = pause_requested.clone();
            events.on_event(move |runtime_event| {
                if let MobsimEvent::BeforeSimStep(event) = &runtime_event.payload {
                    // Pause loop:
                    // as long as pause is requested, wait in short intervals.
                    while pause_requested.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(25));
                    }

                    // Realtime sync starts only after first LinkEnter.
                    if first_link_enter_seen.load(Ordering::Relaxed) {
                        sync_simulation_speed_to_realtime(speed_sync_state.as_ref(), event.time);
                    }

                    // Send simulation time heartbeat to UI.
                    let _ = sender.send(VisualizeEventMessage::BeforeSimStep { time: event.time });
                }
            });
        })
    }

    pub fn run_window(
        receiver: mpsc::Receiver<VisualizeEventMessage>,
        network: Network,
        garage: Garage,
        pause_requested: Arc<AtomicBool>,
    ) {
        // Precompute static lookup data once.
        // This keeps per-frame systems simple and cheap.
        let network_data = NetworkData::from_network(&network);
        let vehicles_data = VehiclesData::from_garage(&garage);

        App::new()
            // Dynamic trip snapshot used by drawing.
            .insert_resource(AllTrips {
                per_vehicle: HashMap::new(),
            })
            // Shared simulation time shown as clock and used for vehicle positions.
            .insert_resource(SimulationClock { time: 0.0 })
            // Inbox for all messages coming from simulation thread.
            .insert_resource(EventsChannel {
                receiver: Mutex::new(receiver),
            })
            // Shared pause state written by UI button.
            .insert_resource(PauseControl { pause_requested })
            .insert_resource(network_data)
            .insert_resource(vehicles_data)
            // Trip builder is non-send because it uses Rc<RefCell>.
            .insert_non_send_resource(TripsBuilderResource {
                builder: Rc::new(RefCell::new(TripsBuilder::new())),
            })
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
                    // Startup order matters:
                    // camera first, then camera fit, then UI widgets.
                    setup_camera,
                    fit_camera_to_network,
                    setup_time_ui,
                    setup_play_pause_button_ui,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    // Update order also matters:
                    // 1) read pause button
                    // 2) process all incoming events
                    // 3) publish latest trips snapshot
                    // 4) draw network and vehicles
                    // 5) refresh time text
                    handle_play_pause_button,
                    process_events_from_channel,
                    update_trips_from_builder,
                    draw_network,
                    draw_vehicles,
                    update_time_ui,
                )
                    .chain(),
            )
            .run();
    }
}

#[derive(Clone)]
struct TraversedLink {
    // Link id that the vehicle traversed.
    link_id: String,
    // Simulation time when vehicle entered this link.
    // This is the key timestamp for later position reconstruction.
    start_time: f32,
}

#[derive(Clone)]
struct Trip {
    // Ordered list of traversed links for one logical trip.
    // Order is important because time/progress is reconstructed from this sequence.
    links: Vec<TraversedLink>,
}

#[derive(Resource)]
struct AllTrips {
    // Final structure consumed by draw_vehicles:
    // vehicle id -> list of finished/in-progress trips.
    per_vehicle: HashMap<String, Vec<Trip>>,
}

#[derive(Resource)]
struct SimulationClock {
    // Latest known simulation time in seconds.
    // Updated from incoming events and shown in the top-right UI clock.
    time: f32,
}

#[derive(Resource)]
struct EventsChannel {
    // Channel receiver for cross-thread event flow.
    // Wrapped in Mutex because systems access it through shared resources.
    receiver: Mutex<mpsc::Receiver<VisualizeEventMessage>>,
}

#[derive(Resource, Default)]
struct NetworkData {
    // node id -> world position
    node_positions: HashMap<String, Vec2>,

    // link id -> (from node id, to node id)
    link_endpoints: HashMap<String, (String, String)>,

    // link id -> free speed limit
    link_freespeed: HashMap<String, f32>,
}

impl NetworkData {
    fn from_network(network: &Network) -> Self {
        // Build compact lookup tables once so drawing code can stay simple.
        let mut data = Self::default();

        for node in network.nodes() {
            data.node_positions.insert(
                node.id.external().to_string(),
                Vec2::new(node.x as f32, node.y as f32),
            );
        }

        for link in network.links() {
            data.link_endpoints.insert(
                link.id.external().to_string(),
                (
                    link.from.external().to_string(),
                    link.to.external().to_string(),
                ),
            );
            data.link_freespeed
                .insert(link.id.external().to_string(), link.freespeed);
        }

        data
    }
}

#[derive(Resource)]
struct ViewSettings {
    // Center point of network bounds.
    center: Vec2,

    // Scale factor so full network fits in the window.
    scale: f32,
}

#[derive(Debug, Clone)]
struct Vehicle {
    // Max allowed speed of this vehicle.
    maximum_velocity: f32,
}

#[derive(Resource, Default)]
struct VehiclesData {
    vehicles: HashMap<String, Vehicle>,
}

impl VehiclesData {
    fn from_garage(garage: &Garage) -> Self {
        // Build direct id->vehicle map once for fast drawing lookup.
        let vehicles = garage
            .vehicles
            .values()
            .map(|vehicle| {
                (
                    vehicle.id.external().to_string(),
                    Vehicle {
                        maximum_velocity: vehicle.max_v,
                    },
                )
            })
            .collect();
        Self { vehicles }
    }
}

struct TripsBuilderResource {
    builder: Rc<RefCell<TripsBuilder>>,
}

#[derive(Component)]
struct SimulationTimeText;

#[derive(Component)]
struct PlayPauseButton;

#[derive(Component)]
struct PlayPauseButtonText;

#[derive(Default)]
struct TripsBuilder {
    // Current active link for each vehicle: (link_id, entered_at_time).
    current_link_per_vehicle: HashMap<String, (String, f32)>,
    // Current active trip under construction for each vehicle.
    current_trip_per_vehicle: HashMap<String, Vec<TraversedLink>>,
    // Finished trips per vehicle.
    per_vehicle: HashMap<String, Vec<Trip>>,
}

impl TripsBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn handle_event(&mut self, event: &VisualizeEventMessage) {
        // Single entry point for event-driven data updates.
        match event {
            VisualizeEventMessage::BeforeSimStep { .. } => {}
            VisualizeEventMessage::LinkEnter {
                time,
                link_id,
                vehicle_id,
            } => self.handle_link_enter(*time, link_id, vehicle_id),
            VisualizeEventMessage::LinkLeave {
                time,
                link_id,
                vehicle_id,
            } => self.handle_link_leave(*time, link_id, vehicle_id),
            VisualizeEventMessage::PersonEntersVehicle { vehicle_id, .. } => {
                self.handle_person_enters(vehicle_id)
            }
            VisualizeEventMessage::PersonLeavesVehicle { vehicle_id, .. } => {
                self.handle_person_leaves(vehicle_id)
            }
            VisualizeEventMessage::Done => {}
        }
    }

    fn handle_link_enter(&mut self, time: u32, link_id: &str, vehicle_id: &str) {
        // Remember where vehicle currently is and when it entered.
        self.current_link_per_vehicle
            .insert(vehicle_id.to_string(), (link_id.to_string(), time as f32));

        // If a trip is active, append this link with start time.
        // Later draw logic turns this into movement along geometry.
        if let Some(current_trip) = self.current_trip_per_vehicle.get_mut(vehicle_id) {
            current_trip.push(TraversedLink {
                link_id: link_id.to_string(),
                start_time: time as f32,
            });
        }
    }

    fn handle_link_leave(&mut self, time: u32, link_id: &str, vehicle_id: &str) {
        // For now we only clear the active-link marker.
        // Local variables are kept for easier debugging and future extensions.
        if let Some((entered_link, start_time)) = self.current_link_per_vehicle.remove(vehicle_id) {
            let end_time = time as f32;
        }
    }

    fn handle_person_enters(&mut self, vehicle_id: &str) {
        // Start (or keep) an active trip bucket for this vehicle.
        self.current_trip_per_vehicle
            .entry(vehicle_id.to_string())
            .or_default();
    }

    fn handle_person_leaves(&mut self, vehicle_id: &str) {
        // Close current trip and move it to finished trips.
        if let Some(trip_links) = self.current_trip_per_vehicle.remove(vehicle_id) {
            if !trip_links.is_empty() {
                self.per_vehicle
                    .entry(vehicle_id.to_string())
                    .or_default()
                    .push(Trip { links: trip_links });
            }
        }
    }

    fn build_all_trips(&self) -> AllTrips {
        // Build immutable snapshot for rendering:
        // includes finished trips and current non-empty in-progress trips.
        let mut per_vehicle = self.per_vehicle.clone();

        for (veh_id, current_links) in &self.current_trip_per_vehicle {
            if !current_links.is_empty() {
                per_vehicle.entry(veh_id.clone()).or_default().push(Trip {
                    links: current_links.clone(),
                });
            }
        }

        AllTrips { per_vehicle }
    }
}

fn setup_camera(mut commands: Commands) {
    // Create a 2D camera and enable pan/zoom interaction.
    commands.spawn((Camera2d, PanCam::default()));
}

fn fit_camera_to_network(
    mut commands: Commands,
    network: Res<NetworkData>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    // Without nodes there is nothing to frame.
    if network.node_positions.is_empty() {
        return;
    }

    let Some(window) = window_query.iter().next() else {
        return;
    };

    // Compute world bounding box of all nodes.
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for pos in network.node_positions.values() {
        min_x = min_x.min(pos.x);
        max_x = max_x.max(pos.x);
        min_y = min_y.min(pos.y);
        max_y = max_y.max(pos.y);
    }

    // Guard against invalid numeric input.
    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return;
    }

    let width = (max_x - min_x).max(f32::EPSILON);
    let height = (max_y - min_y).max(f32::EPSILON);
    let center = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let scale_x = width / window.width().max(1.0);
    let scale_y = height / window.height().max(1.0);
    // Add small padding so the network is not glued to screen borders.
    let scale = (scale_x.max(scale_y) * 1.1).max(1.0);

    commands.insert_resource(ViewSettings { center, scale });
}

fn setup_time_ui(mut commands: Commands) {
    // Simple clock text at top-right corner.
    commands.spawn((
        UiNode {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..Default::default()
        },
        Text::new("00:00:00"),
        TextFont {
            font_size: 22.0,
            ..Default::default()
        },
        TextColor(Color::srgb(1.0, 1.0, 1.0)),
        SimulationTimeText,
    ));
}

fn setup_play_pause_button_ui(mut commands: Commands) {
    // Simple Pause/Play button at top-left corner.
    commands
        .spawn((
            Button,
            UiNode {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                width: Val::Px(120.0),
                height: Val::Px(38.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
            PlayPauseButton,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Pause"),
                TextFont {
                    font_size: 18.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                PlayPauseButtonText,
            ));
        });
}

fn handle_play_pause_button(
    pause_control: Res<PauseControl>,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<PlayPauseButton>),
    >,
    mut text_query: Query<&mut Text, With<PlayPauseButtonText>>,
) {
    // React only to changed interaction state to reduce work.
    for (interaction, mut background) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                // Toggle shared pause flag.
                let currently_paused = pause_control.pause_requested.load(Ordering::Relaxed);
                let new_paused = !currently_paused;
                pause_control
                    .pause_requested
                    .store(new_paused, Ordering::Relaxed);

                // Keep button label in sync with current state.
                if let Some(mut label) = text_query.iter_mut().next() {
                    label.0.clear();
                    label.0.push_str(if new_paused { "Play" } else { "Pause" });
                }

                // Red-ish background means paused.
                *background = if new_paused {
                    BackgroundColor(Color::srgb(0.45, 0.18, 0.18))
                } else {
                    BackgroundColor(Color::srgb(0.2, 0.2, 0.2))
                };
            }
            Interaction::Hovered => {
                *background = BackgroundColor(Color::srgb(0.28, 0.28, 0.28));
            }
            Interaction::None => {
                // Keep stable color based on current pause state.
                let paused = pause_control.pause_requested.load(Ordering::Relaxed);
                *background = if paused {
                    BackgroundColor(Color::srgb(0.45, 0.18, 0.18))
                } else {
                    BackgroundColor(Color::srgb(0.2, 0.2, 0.2))
                };
            }
        }
    }
}

#[derive(Default)]
struct DebugEventStats {
    before_step: u64,
    enters_vehicle: u64,
    link_enter: u64,
    link_leave: u64,
    leaves_vehicle: u64,
    last_reported_hour: Option<u32>,
}

fn process_events_from_channel(
    events_channel: Res<EventsChannel>,
    builder_resource: NonSendMut<TripsBuilderResource>,
    mut clock: ResMut<SimulationClock>,
    mut done: Local<bool>,
    mut stats: Local<DebugEventStats>,
) {
    // Once "Done" was seen, no need to poll again.
    if *done {
        return;
    }

    // Lock receiver and drain all currently queued messages in one update pass.
    let Ok(receiver) = events_channel.receiver.lock() else {
        return;
    };

    loop {
        match receiver.try_recv() {
            Ok(message) => match &message {
                VisualizeEventMessage::Done => {
                    // End of stream marker from simulation.
                    *done = true;
                    break;
                }
                VisualizeEventMessage::BeforeSimStep { time } => {
                    // Clock should never move backwards.
                    clock.time = clock.time.max(*time as f32);
                    stats.before_step += 1;

                    let current_hour = *time / 3600;
                    if stats.last_reported_hour != Some(current_hour) {
                        println!(
                            "[viz][stats] hour={} | before_step={} | enters_vehicle={} | link_enter={} | link_leave={} | leaves_vehicle={}",
                            current_hour,
                            stats.before_step,
                            stats.enters_vehicle,
                            stats.link_enter,
                            stats.link_leave,
                            stats.leaves_vehicle
                        );
                        stats.last_reported_hour = Some(current_hour);
                    }
                }
                VisualizeEventMessage::LinkEnter { time, .. }
                | VisualizeEventMessage::LinkLeave { time, .. }
                | VisualizeEventMessage::PersonEntersVehicle { time, .. }
                | VisualizeEventMessage::PersonLeavesVehicle { time, .. } => {
                    // For trip-related events:
                    // 1) update global time
                    // 2) update builder state (trip data flow)
                    clock.time = clock.time.max(*time as f32);
                    match &message {
                        VisualizeEventMessage::LinkEnter { .. } => stats.link_enter += 1,
                        VisualizeEventMessage::LinkLeave { .. } => stats.link_leave += 1,
                        VisualizeEventMessage::PersonEntersVehicle { .. } => {
                            stats.enters_vehicle += 1
                        }
                        VisualizeEventMessage::PersonLeavesVehicle { .. } => {
                            stats.leaves_vehicle += 1
                        }
                        _ => {}
                    }
                    builder_resource.builder.borrow_mut().handle_event(&message);
                }
            },
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => {
                *done = true;
                break;
            }
        }
    }
}

fn update_time_ui(
    clock: Res<SimulationClock>,
    mut query: Query<&mut Text, With<SimulationTimeText>>,
) {
    // Convert simulation seconds into HH:MM:SS for humans.
    let total_seconds = clock.time.max(0.0) as u32;
    let hours = (total_seconds / 3600) % 24;
    let minutes = (total_seconds / 60) % 60;
    let seconds = total_seconds % 60;
    let time_text = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

    for mut text in &mut query {
        text.0.clear();
        text.0.push_str(&time_text);
    }
}

fn update_trips_from_builder(
    builder_resource: NonSend<TripsBuilderResource>,
    mut trips: ResMut<AllTrips>,
) {
    // Publish latest trip snapshot from builder.
    *trips = builder_resource.builder.borrow().build_all_trips();
}

fn draw_network(mut gizmos: Gizmos, network: Res<NetworkData>, view: Option<Res<ViewSettings>>) {
    // Draw all links as simple white lines.
    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

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
    // If multiple vehicles wait at same node, offset them vertically.
    const WAIT_STACK_OFFSET: f32 = 8.0;

    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

    // Node -> number of already drawn waiting vehicles in current frame.
    let mut waiting_stacks: HashMap<String, u32> = HashMap::new();
    // Current simulation time drives all position calculations below.
    let sim_time = clock.time;

    // Find draw position per vehicle for this exact simulation time.
    for (vehicle_id, trips_for_vehicle) in trips.per_vehicle.iter() {
        if trips_for_vehicle.is_empty() {
            continue;
        }

        // Vehicle speed cap from fleet data. Missing data means "no cap".
        let vehicle_v_max = vehicles
            .vehicles
            .get(vehicle_id)
            .map(|v| v.maximum_velocity)
            .unwrap_or(f32::INFINITY);

        struct VehiclePosition {
            // Position in world coordinates.
            world: Vec2,
            // If Some, vehicle is currently waiting at this node.
            waiting_node: Option<String>,
        }

        struct ScheduledLink {
            // Link geometry.
            from_pos: Vec2,
            to_pos: Vec2,
            // Time window when vehicle moves on this link.
            depart_time: f32,
            arrival_time: f32,
            // Needed for waiting-stack grouping at the arrival node.
            to_node_id: String,
        }

        // We pick the first trip that is active at current simulation time.
        let position_to_draw = trips_for_vehicle.iter().find_map(|trip| {
            if trip.links.is_empty() {
                return None;
            }

            // Build a movement schedule from traversed links + start timestamps.
            let mut schedule = Vec::with_capacity(trip.links.len());
            let mut prev_arrival_time_schedule: Option<f32> = None;

            for traversed_link in &trip.links {
                // Resolve link endpoints by link id.
                let (from_id, to_id) = match network.link_endpoints.get(&traversed_link.link_id) {
                    Some(v) => v.clone(),
                    None => continue,
                };

                // Resolve positions of endpoint nodes.
                let (from_pos, to_pos) = match (
                    network.node_positions.get(&from_id),
                    network.node_positions.get(&to_id),
                ) {
                    (Some(&from), Some(&to)) => (from, to),
                    _ => continue,
                };

                // Travel time = distance / effective speed.
                let link_vector = to_pos - from_pos;
                let link_length = link_vector.length().max(f32::EPSILON);
                let link_v_max = *network
                    .link_freespeed
                    .get(&traversed_link.link_id)
                    .unwrap_or(&f32::INFINITY);
                let v_eff = vehicle_v_max.min(link_v_max);

                if v_eff <= 0.0 {
                    continue;
                }

                let travel_duration = link_length / v_eff;
                let scheduled_start = traversed_link.start_time;
                // Do not depart before previous link actually arrived.
                // This prevents impossible overlap in reconstructed timeline.
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
                return None;
            }

            let trip_start = schedule.first().unwrap().depart_time;
            let trip_end = schedule.last().unwrap().arrival_time;

            // Skip trips that are not active at current time.
            if sim_time < trip_start || sim_time >= trip_end {
                return None;
            }

            let mut prev_arrival_time: Option<f32> = None;
            let mut prev_arrival_pos: Option<Vec2> = None;
            let mut prev_arrival_node_id: Option<String> = None;

            for entry in &schedule {
                // Case 1: vehicle is waiting at node between two links.
                if let (Some(arrival_prev), Some(wait_pos)) = (prev_arrival_time, prev_arrival_pos)
                {
                    if sim_time >= arrival_prev && sim_time < entry.depart_time {
                        return Some(VehiclePosition {
                            world: wait_pos,
                            waiting_node: prev_arrival_node_id.clone(),
                        });
                    }
                }

                // Case 2: vehicle is currently moving on this link.
                if sim_time >= entry.depart_time && sim_time < entry.arrival_time {
                    let travel_duration =
                        (entry.arrival_time - entry.depart_time).max(f32::EPSILON);
                    // Progress from start to end of current link (0.0 .. 1.0).
                    let progress =
                        ((sim_time - entry.depart_time) / travel_duration).clamp(0.0, 1.0);
                    let link_vector = entry.to_pos - entry.from_pos;
                    let position = entry.from_pos + link_vector * progress;
                    return Some(VehiclePosition {
                        world: position,
                        waiting_node: None,
                    });
                }

                prev_arrival_time = Some(entry.arrival_time);
                prev_arrival_pos = Some(entry.to_pos);
                prev_arrival_node_id = Some(entry.to_node_id.clone());
            }

            None
        });

        if let Some(position_info) = position_to_draw {
            let mut position_view = (position_info.world - center) / scale;
            if let Some(node_id) = &position_info.waiting_node {
                // Offset stacked waiting vehicles so all are visible.
                let stack_index = waiting_stacks.entry(node_id.clone()).or_insert(0);
                position_view += Vec2::new(0.0, WAIT_STACK_OFFSET * (*stack_index as f32));
                *stack_index += 1;
            }

            // Draw vehicle as green circle.
            gizmos.circle_2d(position_view, 4.0, Color::srgb(0.0, 1.0, 0.0));
        }
    }
}
