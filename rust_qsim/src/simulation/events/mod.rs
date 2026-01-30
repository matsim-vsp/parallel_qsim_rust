pub mod utils;

use crate::generated::events::MyEvent;
use crate::simulation::id::Id;
use crate::simulation::network::Link;
use crate::simulation::population::InternalPerson;
use crate::simulation::vehicles::InternalVehicle;
use crate::simulation::InternalAttributes;
use derive_builder::Builder;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

pub trait EventTrait: Debug + Any {
    //This can't be a const, because traits with const fields are not dyn compatible.
    fn type_(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn time(&self) -> u32;
    fn attributes(&self) -> &InternalAttributes;
}

type OnEventFn = dyn Fn(&dyn EventTrait) + 'static;

pub type OnEventFnBuilder = dyn FnOnce(&mut EventsManager) + Send;

/// The EventsPublisher holds call-backs for event processing. This might seem a bit odd
/// (in particular in comparison to the Java implementation). The reason is that Rust has no reflection, and this
/// architecture allows compile-time checking of the event types.
#[derive(Default)]
pub struct EventsManager {
    per_type: HashMap<TypeId, Vec<Rc<OnEventFn>>>,
    catch_all: Vec<Box<OnEventFn>>,
    finish: Vec<Box<dyn Fn() + 'static>>,
}

impl Debug for EventsManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EventsPublisher {{ per_type: {:?}, catch_all: {:?}, finish: {:?} }}",
            self.per_type.len(),
            self.catch_all.len(),
            self.finish.len()
        )
    }
}

impl EventsManager {
    pub fn new() -> Self {
        EventsManager {
            per_type: HashMap::new(),
            catch_all: Vec::new(),
            finish: Vec::new(),
        }
    }

    pub fn publish_event(&mut self, event: &dyn EventTrait) {
        let tid = event.as_any().type_id();
        if let Some(list) = self.per_type.get(&tid).cloned() {
            for h in list {
                h(event);
            }
        }
        for h in &self.catch_all {
            h(event);
        }
    }

    pub fn finish(&mut self) {
        for f in self.finish.iter_mut() {
            f()
        }
    }

    /// This function is used to register callbacks for specific event types.
    pub fn on<E, F>(&mut self, f: F)
    where
        E: EventTrait,
        F: Fn(&E) + 'static,
    {
        let type_id = TypeId::of::<E>();
        let entry = self.per_type.entry(type_id).or_default();
        entry.push(Rc::new(move |ev: &dyn EventTrait| {
            if let Some(e) = ev.as_any().downcast_ref::<E>() {
                f(e);
            }
        }));
    }

    /// This function is used to register callbacks for all event types.
    pub fn on_any<F>(&mut self, f: F)
    where
        F: Fn(&dyn EventTrait) + 'static,
    {
        self.catch_all.push(Box::new(f));
    }

    pub fn on_finish<F>(&mut self, f: F)
    where
        F: Fn() + 'static,
    {
        self.finish.push(Box::new(f));
    }
}

/// Events takes a writer. This is the trait for that
pub trait EventsWriter: EventsWriterClone + Send {
    fn write(&self, buf: Vec<u8>);
}

/// Since we want Events to be clonable but we also want the writer to be a dynamic object, we need to
/// specify how to clone the Box<dyn EventsWriter> property in Events. The following three method are
/// concerned about that. Implemented according to https://stackoverflow.com/questions/50017987/cant-clone-vecboxtrait-because-trait-cannot-be-made-into-an-object
///
pub trait EventsWriterClone {
    fn clone_boxed(&self) -> Box<dyn EventsWriter>;
}

/// this implements clone_boxed for all owned EventsWriters e.g. Box<dyn EventsWriter>. The 'static
/// lifetime applies to all values that are owned. See last paragraph (Trait bound) of
/// https://doc.rust-lang.org/rust-by-example/scope/lifetime/static_lifetime.html
impl<T: EventsWriter + Clone + 'static> EventsWriterClone for T {
    fn clone_boxed(&self) -> Box<dyn EventsWriter> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn EventsWriter> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

#[derive(Builder, Debug)]
pub struct GeneralEvent {
    pub time: u32,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl GeneralEvent {
    pub const TYPE: &'static str = "generic";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        GeneralEventBuilder::default()
            .time(time)
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for GeneralEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct ActivityStartEvent {
    pub time: u32,
    pub person: Id<InternalPerson>,
    pub link: Id<Link>,
    pub act_type: Id<String>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl ActivityStartEvent {
    pub const TYPE: &'static str = "actstart";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        ActivityStartEventBuilder::default()
            .time(time)
            .person(Id::create(&event.attributes["person"].as_string()))
            .link(Id::create(&event.attributes["link"].as_string()))
            .act_type(Id::create(&event.attributes["act_type"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for ActivityStartEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct ActivityEndEvent {
    pub time: u32,
    pub person: Id<InternalPerson>,
    pub link: Id<Link>,
    pub act_type: Id<String>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl ActivityEndEvent {
    pub const TYPE: &'static str = "actend";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        ActivityEndEventBuilder::default()
            .time(time)
            .person(Id::create(&event.attributes["person"].as_string()))
            .link(Id::create(&event.attributes["link"].as_string()))
            .act_type(Id::create(&event.attributes["act_type"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for ActivityEndEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct LinkEnterEvent {
    pub time: u32,
    pub link: Id<Link>,
    pub vehicle: Id<InternalVehicle>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl LinkEnterEvent {
    pub const TYPE: &'static str = "entered link";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        LinkEnterEventBuilder::default()
            .time(time)
            .link(Id::create(&event.attributes["link"].as_string()))
            .vehicle(Id::create(&event.attributes["vehicle"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for LinkEnterEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct LinkLeaveEvent {
    pub time: u32,
    pub link: Id<Link>,
    pub vehicle: Id<InternalVehicle>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl LinkLeaveEvent {
    pub const TYPE: &'static str = "left link";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        LinkLeaveEventBuilder::default()
            .time(time)
            .link(Id::create(&event.attributes["link"].as_string()))
            .vehicle(Id::create(&event.attributes["vehicle"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for LinkLeaveEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct VehicleEntersTrafficEvent {
    pub time: u32,
    pub vehicle: Id<InternalVehicle>,
    pub link: Id<Link>,
    pub driver: Id<InternalPerson>,
    pub mode: Id<String>,
    #[builder(default = 1.0)]
    pub relative_position_on_link: f64,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl VehicleEntersTrafficEvent {
    pub const TYPE: &'static str = "vehicle enters traffic";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        VehicleEntersTrafficEventBuilder::default()
            .time(time)
            .vehicle(Id::create(&event.attributes["vehicle"].as_string()))
            .link(Id::create(&event.attributes["link"].as_string()))
            .driver(Id::create(&event.attributes["driver"].as_string()))
            .mode(Id::create(&event.attributes["mode"].as_string()))
            .relative_position_on_link(event.attributes["relative_position_on_link"].as_double())
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for VehicleEntersTrafficEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }

    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct VehicleLeavesTrafficEvent {
    pub time: u32,
    pub vehicle: Id<InternalVehicle>,
    pub link: Id<Link>,
    pub driver: Id<InternalPerson>,
    pub mode: Id<String>,
    #[builder(default = 1.0)]
    pub relative_position_on_link: f64,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl VehicleLeavesTrafficEvent {
    pub const TYPE: &'static str = "vehicle leaves traffic";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        VehicleLeavesTrafficEventBuilder::default()
            .time(time)
            .vehicle(Id::create(&event.attributes["vehicle"].as_string()))
            .link(Id::create(&event.attributes["link"].as_string()))
            .driver(Id::create(&event.attributes["driver"].as_string()))
            .mode(Id::create(&event.attributes["mode"].as_string()))
            .relative_position_on_link(event.attributes["relative_position_on_link"].as_double())
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for VehicleLeavesTrafficEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }

    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct PersonEntersVehicleEvent {
    pub time: u32,
    pub person: Id<InternalPerson>,
    pub vehicle: Id<InternalVehicle>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl PersonEntersVehicleEvent {
    pub const TYPE: &'static str = "PersonEntersVehicle";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        PersonEntersVehicleEventBuilder::default()
            .time(time)
            .person(Id::create(&event.attributes["person"].as_string()))
            .vehicle(Id::create(&event.attributes["vehicle"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for PersonEntersVehicleEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct PersonLeavesVehicleEvent {
    pub time: u32,
    pub person: Id<InternalPerson>,
    pub vehicle: Id<InternalVehicle>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl PersonLeavesVehicleEvent {
    pub const TYPE: &'static str = "PersonLeavesVehicle";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        PersonLeavesVehicleEventBuilder::default()
            .time(time)
            .person(Id::create(&event.attributes["person"].as_string()))
            .vehicle(Id::create(&event.attributes["vehicle"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for PersonLeavesVehicleEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct PersonDepartureEvent {
    pub time: u32,
    pub person: Id<InternalPerson>,
    pub link: Id<Link>,
    pub leg_mode: Id<String>,
    pub routing_mode: Id<String>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl PersonDepartureEvent {
    pub const TYPE: &'static str = "departure";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        PersonDepartureEventBuilder::default()
            .time(time)
            .person(Id::create(&event.attributes["person"].as_string()))
            .link(Id::create(&event.attributes["link"].as_string()))
            .leg_mode(Id::create(&event.attributes["mode"].as_string()))
            .routing_mode(Id::create(&event.attributes["routing_mode"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for PersonDepartureEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct PersonArrivalEvent {
    pub time: u32,
    pub person: Id<InternalPerson>,
    pub link: Id<Link>,
    pub leg_mode: Id<String>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl PersonArrivalEvent {
    pub const TYPE: &'static str = "arrival";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        PersonArrivalEventBuilder::default()
            .time(time)
            .person(Id::create(&event.attributes["person"].as_string()))
            .link(Id::create(&event.attributes["link"].as_string()))
            .leg_mode(Id::create(&event.attributes["mode"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for PersonArrivalEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct TeleportationArrivalEvent {
    pub time: u32,
    pub person: Id<InternalPerson>,
    pub mode: Id<String>,
    pub distance: f64,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl TeleportationArrivalEvent {
    pub const TYPE: &'static str = "travelled";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        TeleportationArrivalEventBuilder::default()
            .time(time)
            .person(Id::create(&event.attributes["person"].as_string()))
            .mode(Id::create(&event.attributes["mode"].as_string()))
            .distance(event.attributes["distance"].as_string().parse().unwrap())
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for TeleportationArrivalEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}

#[derive(Builder, Debug)]
pub struct PtTeleportationArrivalEvent {
    pub time: u32,
    pub person: Id<InternalPerson>,
    pub distance: f64,
    pub mode: Id<String>,
    pub route: Id<String>,
    pub line: Id<String>,
    #[builder(default)]
    pub attributes: InternalAttributes,
}

impl PtTeleportationArrivalEvent {
    pub const TYPE: &'static str = "travelled with pt";
    pub fn from_proto_event(event: &MyEvent, time: u32) -> Self {
        let attrs = InternalAttributes::from(&event.attributes);
        assert!(event.r#type.eq(Self::TYPE));
        PtTeleportationArrivalEventBuilder::default()
            .time(time)
            .person(Id::create(&event.attributes["person"].as_string()))
            .distance(event.attributes["distance"].as_string().parse().unwrap())
            .mode(Id::create(&event.attributes["mode"].as_string()))
            .route(Id::create(&event.attributes["route"].as_string()))
            .line(Id::create(&event.attributes["line"].as_string()))
            .attributes(attrs)
            .build()
            .unwrap()
    }
}

impl EventTrait for PtTeleportationArrivalEvent {
    fn type_(&self) -> &'static str {
        Self::TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn time(&self) -> u32 {
        self.time
    }
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }
}
