use crate::simulation::events::{
    EventHandlerRegisterFn, EventsManager, LinkEnterEvent, LinkLeaveEvent, PersonEntersVehicleEvent,
    PersonLeavesVehicleEvent,
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
use std::sync::{mpsc, Mutex};

const WAIT_STACK_OFFSET: f32 = 8.0;

#[derive(Debug, Clone)]
pub enum VisualizeEventMessage {
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

pub struct VisualizeEvents;

impl VisualizeEvents {
    pub fn register_fn(sender: mpsc::Sender<VisualizeEventMessage>) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            let sender_on_event = sender.clone();
            events.on_any(move |event| {

                let msg = if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
                    // println!(
                    //     "[EVENT] LinkEnter  | t={} | link={} | vehicle={}",
                    //     e.time,
                    //     e.link.external(),
                    //     e.vehicle.external()
                    // );

                    Some(VisualizeEventMessage::LinkEnter {
                        time: e.time,
                        link_id: e.link.external().to_string(),
                        vehicle_id: e.vehicle.external().to_string(),
                    })

                } else if let Some(e) = event.as_any().downcast_ref::<LinkLeaveEvent>() {
                    // println!(
                    //     "[EVENT] LinkLeave  | t={} | link={} | vehicle={}",
                    //     e.time,
                    //     e.link.external(),
                    //     e.vehicle.external()
                    // );

                    Some(VisualizeEventMessage::LinkLeave {
                        time: e.time,
                        link_id: e.link.external().to_string(),
                        vehicle_id: e.vehicle.external().to_string(),
                    })

                } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
                    // println!(
                    //     "[EVENT] PersonEntersVehicle | t={} | vehicle={}",
                    //     e.time,
                    //     e.vehicle.external()
                    // );

                    Some(VisualizeEventMessage::PersonEntersVehicle {
                        time: e.time,
                        vehicle_id: e.vehicle.external().to_string(),
                    })

                } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
                    // println!(
                    //     "[EVENT] PersonLeavesVehicle | t={} | vehicle={}",
                    //     e.time,
                    //     e.vehicle.external()
                    // );

                    Some(VisualizeEventMessage::PersonLeavesVehicle {
                        time: e.time,
                        vehicle_id: e.vehicle.external().to_string(),
                    })

                } else {
                    // println!(
                    //     "[EVENT] Other event type: {}",
                    //     std::any::type_name_of_val(event)
                    // );
                    None
                };

                if let Some(message) = msg {
                    let _ = sender_on_event.send(message);
                }
            });

            let sender_on_finish = sender.clone();
            events.on_finish(move || {
                let _ = sender_on_finish.send(VisualizeEventMessage::Done);
            });
        })
    }

    pub fn run_window(
        receiver: mpsc::Receiver<VisualizeEventMessage>,
        network: Network,
        garage: Garage,
    ) {
        let network_data = NetworkData::from_network(&network);
        let vehicles_data = VehiclesData::from_garage(&garage);

        App::new()
            .insert_resource(AllTrips {
                per_vehicle: HashMap::new(),
            })
            .insert_resource(SimulationClock { time: 0.0 })
            .insert_resource(EventsChannel {
                receiver: Mutex::new(receiver),
            })
            .insert_resource(network_data)
            .insert_resource(vehicles_data)
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
    link_id: String,
    start_time: f32,
}

#[derive(Clone)]
struct Trip {
    links: Vec<TraversedLink>,
}

#[derive(Resource)]
struct AllTrips {
    per_vehicle: HashMap<String, Vec<Trip>>,
}

#[derive(Resource)]
struct SimulationClock {
    time: f32,
}

#[derive(Resource)]
struct EventsChannel {
    receiver: Mutex<mpsc::Receiver<VisualizeEventMessage>>,
}

#[derive(Resource, Default)]
struct NetworkData {
    node_positions: HashMap<String, Vec2>,
    link_endpoints: HashMap<String, (String, String)>,
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

#[derive(Resource)]
struct ViewSettings {
    center: Vec2,
    scale: f32,
}

#[derive(Debug, Clone)]
struct Vehicle {
    maximum_velocity: f32,
}

#[derive(Resource, Default)]
struct VehiclesData {
    vehicles: HashMap<String, Vehicle>,
}

impl VehiclesData {
    fn from_garage(garage: &Garage) -> Self {
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

#[derive(Default)]
struct TripsBuilder {
    current_link_per_vehicle: HashMap<String, (String, f32)>,
    current_trip_per_vehicle: HashMap<String, Vec<TraversedLink>>,
    per_vehicle: HashMap<String, Vec<Trip>>,
}

impl TripsBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn handle_event(&mut self, event: &VisualizeEventMessage) {
        match event {
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
        self.current_link_per_vehicle
            .insert(vehicle_id.to_string(), (link_id.to_string(), time as f32));

        // Add the traversed link immediately on enter so the vehicle becomes visible
        // even before the corresponding leave event arrives.
        if let Some(current_trip) = self.current_trip_per_vehicle.get_mut(vehicle_id) {
            current_trip.push(TraversedLink {
                link_id: link_id.to_string(),
                start_time: time as f32,
            });
        }
    }

    fn handle_link_leave(&mut self, time: u32, link_id: &str, vehicle_id: &str) {
        if let Some((entered_link, start_time)) = self.current_link_per_vehicle.remove(vehicle_id)
        {
            let end_time = time as f32;
            if entered_link == link_id && end_time >= start_time {
                // Link already added on enter; leave only validates/removes current open link.
            }
        }
    }

    fn handle_person_enters(&mut self, vehicle_id: &str) {
        self.current_trip_per_vehicle
            .entry(vehicle_id.to_string())
            .or_default();
    }

    fn handle_person_leaves(&mut self, vehicle_id: &str) {
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
    commands.spawn((Camera2d, PanCam::default()));
}

fn fit_camera_to_network(
    mut commands: Commands,
    network: Res<NetworkData>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    if network.node_positions.is_empty() {
        return;
    }

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
    let center = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let scale_x = width / window.width().max(1.0);
    let scale_y = height / window.height().max(1.0);
    let scale = (scale_x.max(scale_y) * 1.1).max(1.0);

    commands.insert_resource(ViewSettings { center, scale });
}

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

fn process_events_from_channel(
    events_channel: Res<EventsChannel>,
    builder_resource: NonSendMut<TripsBuilderResource>,
    mut clock: ResMut<SimulationClock>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }

    let Ok(receiver) = events_channel.receiver.lock() else {
        return;
    };

    loop {
        match receiver.try_recv() {
            Ok(message) => match &message {
                VisualizeEventMessage::Done => {
                    *done = true;
                    break;
                }
                VisualizeEventMessage::LinkEnter { time, .. }
                | VisualizeEventMessage::LinkLeave { time, .. }
                | VisualizeEventMessage::PersonEntersVehicle { time, .. }
                | VisualizeEventMessage::PersonLeavesVehicle { time, .. } => {
                    clock.time = clock.time.max(*time as f32);
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
    *trips = builder_resource.builder.borrow().build_all_trips();
}

fn draw_network(mut gizmos: Gizmos, network: Res<NetworkData>, view: Option<Res<ViewSettings>>) {
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
    let (center, scale) = if let Some(view) = view {
        (view.center, view.scale.max(f32::EPSILON))
    } else {
        (Vec2::ZERO, 1.0)
    };

    let mut waiting_stacks: HashMap<String, u32> = HashMap::new();
    let sim_time = clock.time;

    for (vehicle_id, trips_for_vehicle) in trips.per_vehicle.iter() {
        if trips_for_vehicle.is_empty() {
            continue;
        }

        let vehicle_v_max = vehicles
            .vehicles
            .get(vehicle_id)
            .map(|v| v.maximum_velocity)
            .unwrap_or(f32::INFINITY);

        struct VehiclePosition {
            world: Vec2,
            waiting_node: Option<String>,
        }

        struct ScheduledLink {
            from_pos: Vec2,
            to_pos: Vec2,
            depart_time: f32,
            arrival_time: f32,
            to_node_id: String,
        }

        let position_to_draw = trips_for_vehicle.iter().find_map(|trip| {
            if trip.links.is_empty() {
                return None;
            }

            let mut schedule = Vec::with_capacity(trip.links.len());
            let mut prev_arrival_time_schedule: Option<f32> = None;

            for traversed_link in &trip.links {
                let (from_id, to_id) = match network.link_endpoints.get(&traversed_link.link_id) {
                    Some(v) => v.clone(),
                    None => continue,
                };

                let (from_pos, to_pos) = match (
                    network.node_positions.get(&from_id),
                    network.node_positions.get(&to_id),
                ) {
                    (Some(&from), Some(&to)) => (from, to),
                    _ => continue,
                };

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

            if sim_time < trip_start || sim_time >= trip_end {
                return None;
            }

            let mut prev_arrival_time: Option<f32> = None;
            let mut prev_arrival_pos: Option<Vec2> = None;
            let mut prev_arrival_node_id: Option<String> = None;

            for entry in &schedule {
                if let (Some(arrival_prev), Some(wait_pos)) = (prev_arrival_time, prev_arrival_pos)
                {
                    if sim_time >= arrival_prev && sim_time < entry.depart_time {
                        return Some(VehiclePosition {
                            world: wait_pos,
                            waiting_node: prev_arrival_node_id.clone(),
                        });
                    }
                }

                if sim_time >= entry.depart_time && sim_time < entry.arrival_time {
                    let travel_duration =
                        (entry.arrival_time - entry.depart_time).max(f32::EPSILON);
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
                let stack_index = waiting_stacks.entry(node_id.clone()).or_insert(0);
                position_view += Vec2::new(0.0, WAIT_STACK_OFFSET * (*stack_index as f32));
                *stack_index += 1;
            }

            // println!(
            //     "Drawing vehicle {} at sim_time {:.2} at position {:?}",
            //     vehicle_id,
            //     sim_time,
            //     position_info.world
            // );

            gizmos.circle_2d(position_view, 4.0, Color::srgb(0.0, 1.0, 0.0));
        }
    }
}
