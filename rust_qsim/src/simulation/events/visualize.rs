use crate::simulation::events::{
    EventHandlerRegisterFn, EventsManager, LinkEnterEvent, LinkLeaveEvent,
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

// Define time scale. 1.0 means realtime and 10.0 means 10 times realtime
const REALTIME_SCALE: f64 = 40.0;

// Events that come from the simulation
#[derive(Debug, Clone)]
pub enum VisualizeEventMessage {
    BeforeSimStep {
        time: u32,
    },
    LinkEnter {
        time: u32,
        link_id: String,
        vehicle_id: String,
    },
    LinkLeave {
        time: u32,
        link_id: String,
        vehicle_id: String,
    },
    PersonEntersVehicle {
        time: u32,
        vehicle_id: String,
    },
    PersonLeavesVehicle {
        time: u32,
        vehicle_id: String,
    },
    Done,
}

// Zustandsdaten für die Echtzeit-Synchronisierung.
// Speichert den Startpunkt, von dem aus Soll- und Ist-Zeit verglichen werden.
#[derive(Default)]
struct RealtimeSyncState {
    // realtime when the sync starts
    real_time_sync_start: Option<Instant>,
    // simtime when the sync starts
    sim_time_sync_start: Option<u32>,
}

// This method controls the time so that the simulation runs in realtime
fn sync_to_realtime(sync_state_mutex: &Mutex<RealtimeSyncState>, current_sim_time: u32) {
    // Return if the time scale is not valid
    if REALTIME_SCALE <= 0.0 {
        return;
    }

    // lock sync state
    let Ok(mut sync_state) = sync_state_mutex.lock() else {
        return;
    };

    // First call: set the sim/real time start
    if sync_state.sim_time_sync_start.is_none() || sync_state.real_time_sync_start.is_none() {
        sync_state.sim_time_sync_start = Some(current_sim_time);
        sync_state.real_time_sync_start = Some(Instant::now());
        return;
    }

    // get sim and real time
    let Some(sim_time_at_start) = sync_state.sim_time_sync_start else {
        return;
    };
    let Some(real_time_at_start) = sync_state.real_time_sync_start.as_ref() else {
        return;
    };

    // elapsed sim time since the real time sync started
    let elapsed_sim_seconds = (current_sim_time - sim_time_at_start) as f64;

    // calculates how much time should have elapsed since the sync started
    let expected_real_seconds = elapsed_sim_seconds / REALTIME_SCALE;

    // elapsed real time since the real time sync started
    let actual_real_seconds = real_time_at_start.elapsed().as_secs_f64();

    // Sleep if sim time is too fast. otherwise: do nothing
    if expected_real_seconds > actual_real_seconds {
        thread::sleep(Duration::from_secs_f64(
            expected_real_seconds - actual_real_seconds,
        ));
    }
}

pub struct VisualizeEvents;

impl VisualizeEvents {
    // callback for events
    pub fn register_fn(
        sender: mpsc::Sender<VisualizeEventMessage>,
        first_link_enter_seen: Arc<AtomicBool>,
    ) -> Box<EventHandlerRegisterFn> {
        Box::new(move |event_manager: &mut EventsManager| {
            let event_sender = sender.clone();
            let first_link_enter_seen_clone = first_link_enter_seen.clone();

            // Check via downcast the event type
            event_manager.on_any(move |event| {
                let viz_message = if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
                    // store the first link enter event to start real time sync
                    first_link_enter_seen_clone.store(true, Ordering::Relaxed);
                    Some(VisualizeEventMessage::LinkEnter {
                        time: e.time,
                        link_id: e.link.external().to_string(),
                        vehicle_id: e.vehicle.external().to_string(),
                    })
                } else if let Some(e) = event.as_any().downcast_ref::<LinkLeaveEvent>() {
                    Some(VisualizeEventMessage::LinkLeave {
                        time: e.time,
                        link_id: e.link.external().to_string(),
                        vehicle_id: e.vehicle.external().to_string(),
                    })
                } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
                    Some(VisualizeEventMessage::PersonEntersVehicle {
                        time: e.time,
                        vehicle_id: e.vehicle.external().to_string(),
                    })
                } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
                    Some(VisualizeEventMessage::PersonLeavesVehicle {
                        time: e.time,
                        vehicle_id: e.vehicle.external().to_string(),
                    })
                } else {
                    None
                };

                // Nachricht nur senden, wenn sie tatsächlich erzeugt wurde.
                // Fehler beim Senden (z.B. Empfänger schon geschlossen) werden ignoriert,
                // damit die Simulation auch nach dem Schließen des Fensters weiterläuft.
                if let Some(message) = viz_message {
                    let _ = event_sender.send(message);
                }
            });

            // Send Done when the simulation is done
            let finish_sender = sender.clone();
            event_manager.on_finish(move || {
                let _ = finish_sender.send(VisualizeEventMessage::Done);
            });
        })
    }

    // Mobsim callback
    pub fn register_mobsim_fn(
        sender: mpsc::Sender<VisualizeEventMessage>,
        first_link_enter_seen: Arc<AtomicBool>,
    ) -> Box<MobsimListenerRegisterFn> {
        let sync_state = Arc::new(Mutex::new(RealtimeSyncState::default()));

        Box::new(move |mobsim_events: &mut MobsimEventsManager| {
            let first_link_enter_seen = first_link_enter_seen.clone();
            let sync_state = sync_state.clone();
            mobsim_events.on_event(move |mobsim_event| {
                // Check if this is a `BeforeSimStep` event
                if let MobsimEvent::BeforeSimStep(step_info) = &mobsim_event.payload {
                    // time sync only when the first vehicle is on the network
                    if first_link_enter_seen.load(Ordering::Relaxed) {
                        sync_to_realtime(sync_state.as_ref(), step_info.time);
                    }

                    // send current time to the UI to display the time
                    let _ = sender.send(VisualizeEventMessage::BeforeSimStep {
                        time: step_info.time,
                    });
                }
            });
        })
    }

    pub fn run_window(
        receiver: mpsc::Receiver<VisualizeEventMessage>,
        network: Network,
        garage: Garage,
    ) {
        // Netzwerk- und Fahrzeugdaten einmalig beim Start in schnelle Lookup-Tabellen umwandeln.
        // Alle Zeichensysteme lesen daraus jedes Frame, ohne die vollen Originaldaten zu brauchen.
        let network_data = NetworkData::from_network(&network);
        let vehicle_data = VehicleData::from_garage(&garage);

        App::new()
            .insert_resource(AllTrips {
                per_vehicle: HashMap::new(),
            })
            .insert_resource(SimulationClock { time: 0.0 })
            .insert_resource(EventsChannel {
                receiver: Mutex::new(receiver),
            })
            .insert_resource(network_data)
            .insert_resource(vehicle_data)
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
                (setup_camera, fit_camera_to_network, setup_time_ui).chain(),
            )
            .add_systems(
                Update,
                (
                    process_channel_events,
                    update_all_trips,
                    draw_network,
                    draw_vehicles,
                    update_time_ui,
                )
                    .chain(),
            )
            .run();
    }
}

// saves a single link that has been visited
#[derive(Clone)]
struct TraversedLink {
    link_id: String,
    // sim time when the vehicle starts on this link
    entry_time: f32,
}

// represents a trip with all TraversedLinks
#[derive(Clone)]
struct Trip {
    traversed_links: Vec<TraversedLink>,
    // time where the vehicle leaves the network (PersonLeavesVehicle)
    leave_time: Option<f32>,
}

// bevy ressource with all trips
#[derive(Resource)]
struct AllTrips {
    // vehicle-ID → list with alle trips
    per_vehicle: HashMap<String, Vec<Trip>>,
}

// bevy ressource with current sim time
#[derive(Resource)]
struct SimulationClock {
    time: f32,
}

#[derive(Resource)]
struct EventsChannel {
    receiver: Mutex<mpsc::Receiver<VisualizeEventMessage>>,
}

// Bevy-Ressource with network data
#[derive(Resource, Default)]
struct NetworkData {
    // note id -> position (x, y)
    node_positions: HashMap<String, Vec2>,
    // link id -> (start node id, end node id)
    link_endpoints: HashMap<String, (String, String)>,
    // link id -> freespeed
    link_freespeed: HashMap<String, f32>,
}

impl NetworkData {
    fn from_network(network: &Network) -> Self {
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

// Bevy camera ressource
#[derive(Resource)]
struct ViewSettings {
    // network center
    network_center: Vec2,
    // scale factor
    scale: f32,
}

// vehicle max speed
#[derive(Debug, Clone)]
struct Vehicle {
    max_speed: f32,
}

// bevy ressource with all vehicle max speeds
#[derive(Resource, Default)]
struct VehicleData {
    vehicles: HashMap<String, Vehicle>,
}

impl VehicleData {
    fn from_garage(garage: &Garage) -> Self {
        let vehicles = garage
            .vehicles
            .values()
            .map(|v| (v.id.external().to_string(), Vehicle { max_speed: v.max_v }))
            .collect();
        Self { vehicles }
    }
}

struct TripsBuilderResource {
    builder: Rc<RefCell<TripsBuilder>>,
}

#[derive(Component)]
struct SimulationTimeText;

// Builds the current trips
#[derive(Default)]
struct TripsBuilder {
    // current visited link per vehicle: vehicle id -> (link id, start time)
    current_link_per_vehicle: HashMap<String, (String, f32)>,
    // current trips per vehicle
    active_trip_per_vehicle: HashMap<String, Vec<TraversedLink>>,
    // finished trips per vehicle
    finished_trips_per_vehicle: HashMap<String, Vec<Trip>>,
}

impl TripsBuilder {
    fn new() -> Self {
        Self::default()
    }

    // entry point. distribute the events to the corresponding method
    fn handle_event(&mut self, message: &VisualizeEventMessage) {
        match message {
            VisualizeEventMessage::BeforeSimStep { .. } => {}
            VisualizeEventMessage::LinkEnter {
                time,
                link_id,
                vehicle_id,
            } => self.on_link_enter(*time, link_id, vehicle_id),
            VisualizeEventMessage::LinkLeave {
                time,
                link_id,
                vehicle_id,
            } => self.on_link_leave(*time, link_id, vehicle_id),
            VisualizeEventMessage::PersonEntersVehicle { vehicle_id, .. } => {
                self.on_person_enters(vehicle_id)
            }
            VisualizeEventMessage::PersonLeavesVehicle { time, vehicle_id } => {
                self.on_person_leaves(*time, vehicle_id)
            }
            VisualizeEventMessage::Done => {}
        }
    }

    fn on_link_enter(&mut self, time: u32, link_id: &str, vehicle_id: &str) {
        self.current_link_per_vehicle
            .insert(vehicle_id.to_string(), (link_id.to_string(), time as f32));

        if let Some(active_trip) = self.active_trip_per_vehicle.get_mut(vehicle_id) {
            active_trip.push(TraversedLink {
                link_id: link_id.to_string(),
                entry_time: time as f32,
            });
        }
    }

    fn on_link_leave(&mut self, time: u32, _link_id: &str, vehicle_id: &str) {
        if let Some((_left_link, _entry_time)) = self.current_link_per_vehicle.remove(vehicle_id) {
            let _exit_time = time as f32;
        }
    }

    fn on_person_enters(&mut self, vehicle_id: &str) {
        self.active_trip_per_vehicle
            .entry(vehicle_id.to_string())
            .or_default();
    }

    fn on_person_leaves(&mut self, time: u32, vehicle_id: &str) {
        if let Some(traversed_links) = self.active_trip_per_vehicle.remove(vehicle_id) {
            if !traversed_links.is_empty() {
                self.finished_trips_per_vehicle
                    .entry(vehicle_id.to_string())
                    .or_default()
                    .push(Trip {
                        traversed_links,
                        leave_time: Some(time as f32),
                    });
            }
        }
    }

    fn build_all_trips(&self) -> AllTrips {
        let mut per_vehicle = self.finished_trips_per_vehicle.clone();

        for (vehicle_id, active_links) in &self.active_trip_per_vehicle {
            if !active_links.is_empty() {
                per_vehicle
                    .entry(vehicle_id.clone())
                    .or_default()
                    .push(Trip {
                        traversed_links: active_links.clone(),
                        leave_time: None,
                    });
            }
        }

        AllTrips { per_vehicle }
    }
}

// setup camera
fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, PanCam::default()));
}

// calculates the the center of the network and the zoom factor
fn fit_camera_to_network(
    mut commands: Commands,
    network: Res<NetworkData>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    if network.node_positions.is_empty() {
        return;
    }

    // get window size
    let Some(window) = window_query.iter().next() else {
        return;
    };

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

    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return;
    }

    let width = (max_x - min_x).max(f32::EPSILON);
    let height = (max_y - min_y).max(f32::EPSILON);
    let network_center = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let scale_x = width / window.width().max(1.0);
    let scale_y = height / window.height().max(1.0);
    // add 10 percent margin
    let scale = (scale_x.max(scale_y) * 1.1).max(1.0);

    commands.insert_resource(ViewSettings {
        network_center,
        scale,
    });
}

// creates the time in the UI
fn setup_time_ui(mut commands: Commands) {
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

// Liest alle aktuell im Kanal wartenden Nachrichten aus und verarbeitet sie.
// Wird jedes Frame aufgerufen, bevor Snapshot und Zeichnung stattfinden.
//
// Datenfluss:
//   Kanal-Nachricht
//     → Simulationsuhr aktualisieren
//     → TripsBuilder-Zustand ändern (Fahrt öffnen / Link hinzufügen / Fahrt schließen)
//     → im nächsten Schritt als Snapshot für die Zeichnung bereitstellen
fn process_channel_events(
    events_channel: Res<EventsChannel>,
    builder_res: NonSendMut<TripsBuilderResource>,
    mut clock: ResMut<SimulationClock>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }

    // Mutex sperren, um den Empfänger zu nutzen. Bei Fehler diesen Frame überspringen.
    let Ok(receiver) = events_channel.receiver.lock() else {
        return;
    };

    // Alle aktuell verfügbaren Nachrichten ohne Blockierung aus dem Kanal lesen.
    // So bleibt die UI möglichst aktuell, ohne auf neue Nachrichten zu warten.
    loop {
        match receiver.try_recv() {
            Ok(message) => match &message {
                VisualizeEventMessage::Done => {
                    // Simulation abgeschlossen – ab jetzt keine weiteren Nachrichten mehr.
                    *done = true;
                    break;
                }
                VisualizeEventMessage::BeforeSimStep { time } => {
                    // Uhr vorwärts stellen, aber niemals rückwärts
                    // (Nachrichten können leicht verzögert ankommen).
                    clock.time = clock.time.max(*time as f32);
                }
                // Alle vier fahrtbezogenen Nachrichtentypen werden gleich behandelt:
                // 1) Uhr vorwärts stellen, 2) Statistik aktualisieren,
                // 3) Nachricht an den TripsBuilder weitergeben, der den Fahrt-Zustand ändert.
                VisualizeEventMessage::LinkEnter { time, .. }
                | VisualizeEventMessage::LinkLeave { time, .. }
                | VisualizeEventMessage::PersonEntersVehicle { time, .. }
                | VisualizeEventMessage::PersonLeavesVehicle { time, .. } => {
                    clock.time = clock.time.max(*time as f32);
                    // TripsBuilder aktualisiert seine internen Maps
                    // (Fahrt öffnen, Link hinzufügen oder Fahrt abschließen).
                    builder_res.builder.borrow_mut().handle_event(&message);
                }
            },
            // Kanal ist leer – alle verfügbaren Nachrichten wurden in diesem Frame verarbeitet.
            Err(mpsc::TryRecvError::Empty) => break,
            // Sender wurde fallen gelassen (Simulations-Thread beendet) → als fertig markieren.
            Err(mpsc::TryRecvError::Disconnected) => {
                *done = true;
                break;
            }
        }
    }
}

fn update_all_trips(builder_res: NonSend<TripsBuilderResource>, mut trips: ResMut<AllTrips>) {
    *trips = builder_res.builder.borrow().build_all_trips();
}

fn draw_network(mut gizmos: Gizmos, network: Res<NetworkData>, view: Option<Res<ViewSettings>>) {
    let (center, scale) = if let Some(v) = view {
        (v.network_center, v.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

    for (_link_id, (from_id, to_id)) in &network.link_endpoints {
        if let (Some(from_pos), Some(to_pos)) = (
            network.node_positions.get(from_id),
            network.node_positions.get(to_id),
        ) {
            gizmos.line_2d(
                (*from_pos - center) / scale,
                (*to_pos - center) / scale,
                Color::srgb(1.0, 1.0, 1.0),
            );
        }
    }
}

fn draw_vehicles(
    mut gizmos: Gizmos,
    trips: Res<AllTrips>,
    network: Res<NetworkData>,
    vehicles: Res<VehicleData>,
    view: Option<Res<ViewSettings>>,
    clock: Res<SimulationClock>,
) {
    const WAIT_STACK_OFFSET: f32 = 8.0;

    let (center, scale) = if let Some(v) = view {
        (v.network_center, v.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

    let mut waiting_stacks: HashMap<String, u32> = HashMap::new();

    let current_sim_time = clock.time;

    for (vehicle_id, vehicle_trips) in trips.per_vehicle.iter() {
        // Fahrzeuge ohne Fahrten können keine Position haben.
        if vehicle_trips.is_empty() {
            continue;
        }

        let vehicle_max_speed = vehicles
            .vehicles
            .get(vehicle_id)
            .map(|v| v.max_speed)
            .unwrap_or(f32::INFINITY);

        struct VehiclePosition {
            world: Vec2,

            waiting_node: Option<String>,
        }

        struct ScheduledLink {
            from_pos: Vec2,
            to_pos: Vec2,
            departure_time: f32,
            arrival_time: f32,

            to_node_id: String,
        }

        let computed_position = vehicle_trips.iter().find_map(|trip| {
            if trip.traversed_links.is_empty() {
                return None;
            }

            let mut schedule = Vec::with_capacity(trip.traversed_links.len());
            let mut prev_arrival_time: Option<f32> = None;

            for traversed_link in &trip.traversed_links {
                let (from_id, to_id) = match network.link_endpoints.get(&traversed_link.link_id) {
                    Some(v) => v.clone(),
                    None => continue,
                };

                let (from_pos, to_pos) = match (
                    network.node_positions.get(&from_id),
                    network.node_positions.get(&to_id),
                ) {
                    (Some(&f), Some(&t)) => (f, t),
                    _ => continue,
                };

                let link_vector = to_pos - from_pos;
                let link_length = link_vector.length().max(f32::EPSILON);
                let link_freespeed = *network
                    .link_freespeed
                    .get(&traversed_link.link_id)
                    .unwrap_or(&f32::INFINITY);
                let effective_speed = vehicle_max_speed.min(link_freespeed);

                if effective_speed <= 0.0 {
                    continue;
                }

                let travel_duration = link_length / effective_speed;
                let scheduled_start = traversed_link.entry_time;

                let departure_time = match prev_arrival_time {
                    Some(prev_arrival) => scheduled_start.max(prev_arrival),
                    None => scheduled_start,
                };
                let arrival_time = departure_time + travel_duration;

                schedule.push(ScheduledLink {
                    from_pos,
                    to_pos,
                    departure_time,
                    arrival_time,
                    to_node_id: to_id.clone(),
                });

                prev_arrival_time = Some(arrival_time);
            }

            if schedule.is_empty() {
                return None;
            }

            let trip_start = schedule.first().unwrap().departure_time;

            let freespeed_end = schedule.last().unwrap().arrival_time;

            let actual_end = trip.leave_time.unwrap_or(f32::INFINITY);

            if current_sim_time < trip_start {
                return None;
            }

            if current_sim_time >= actual_end {
                return None;
            }

            if current_sim_time >= freespeed_end {
                let last = schedule.last().unwrap();
                return Some(VehiclePosition {
                    world: last.to_pos,
                    waiting_node: Some(last.to_node_id.clone()),
                });
            }

            let mut prev_link_arrival_time: Option<f32> = None;
            let mut prev_link_arrival_pos: Option<Vec2> = None;
            let mut prev_link_arrival_node: Option<String> = None;

            for entry in &schedule {
                if let (Some(prev_arrival), Some(wait_pos)) =
                    (prev_link_arrival_time, prev_link_arrival_pos)
                {
                    if current_sim_time >= prev_arrival && current_sim_time < entry.departure_time {
                        return Some(VehiclePosition {
                            world: wait_pos,
                            waiting_node: prev_link_arrival_node.clone(),
                        });
                    }
                }

                if current_sim_time >= entry.departure_time && current_sim_time < entry.arrival_time
                {
                    let link_travel_duration =
                        (entry.arrival_time - entry.departure_time).max(f32::EPSILON);
                    let progress = ((current_sim_time - entry.departure_time)
                        / link_travel_duration)
                        .clamp(0.0, 1.0);
                    let link_vector = entry.to_pos - entry.from_pos;
                    let world_pos = entry.from_pos + link_vector * progress;
                    return Some(VehiclePosition {
                        world: world_pos,
                        waiting_node: None,
                    });
                }

                prev_link_arrival_time = Some(entry.arrival_time);
                prev_link_arrival_pos = Some(entry.to_pos);
                prev_link_arrival_node = Some(entry.to_node_id.clone());
            }

            None
        });

        if let Some(position_info) = computed_position {
            let mut screen_pos = (position_info.world - center) / scale;

            if let Some(node_id) = &position_info.waiting_node {
                let stack_index = waiting_stacks.entry(node_id.clone()).or_insert(0);
                screen_pos += Vec2::new(0.0, WAIT_STACK_OFFSET * (*stack_index as f32));
                *stack_index += 1;
            }

            gizmos.circle_2d(screen_pos, 4.0, Color::srgb(0.0, 1.0, 0.0));
        }
    }
}

fn update_time_ui(
    clock: Res<SimulationClock>,
    mut query: Query<&mut Text, With<SimulationTimeText>>,
) {
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
