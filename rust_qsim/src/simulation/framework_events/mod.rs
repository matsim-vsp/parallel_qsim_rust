pub type QSimId = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MobsimEvent {
    Initialized(),
    BeforeSimStep(MobsimTimeEvent),
    AfterSimStep(MobsimTimeEvent),
    BeforeCleanup(),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MobsimTimeEvent {
    pub time: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ControllerEvent {
    Startup(GeneralControllerEvent),
    IterationStarts(GeneralControllerEvent),
    BeforeMobsim(GeneralControllerEvent),
    AfterMobsim(GeneralControllerEvent),
    Scoring(GeneralControllerEvent),
    Replanning(GeneralControllerEvent),
    IterationEnds(GeneralControllerEvent),
    Shutdown(GeneralControllerEvent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneralControllerEvent {
    pub last_iteration: bool,
    // something like "matsim services" in java
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeEvent<E> {
    pub meta: EventMeta,
    pub payload: E,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventOrigin {
    Controller,
    Partition(QSimId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventMeta {
    pub origin: EventOrigin,
    pub iteration: u32,
    pub seq_no: u64,
}

pub type MobsimRuntimeEvent = RuntimeEvent<MobsimEvent>;
pub type ControllerRuntimeEvent = RuntimeEvent<ControllerEvent>;

pub type MobsimEventsManager = FrameworkEventsManager<MobsimEvent>;
pub type ControllerEventsManager = FrameworkEventsManager<ControllerEvent>;

pub type MobsimListenerRegistrator = dyn FnOnce(&mut MobsimEventsManager) + Send;
pub type ControllerListenerRegistrator = dyn FnOnce(&mut ControllerEventsManager) + Send;

#[derive(Debug, Clone, Copy)]
struct EventRuntimeState {
    origin: EventOrigin,
    iteration: u32,
    next_seq_no: u64,
}

impl EventRuntimeState {
    fn new(origin: EventOrigin, iteration: u32) -> Self {
        Self {
            origin,
            iteration,
            next_seq_no: 0,
        }
    }

    fn wrap<E>(&mut self, payload: E) -> RuntimeEvent<E> {
        let event = RuntimeEvent {
            meta: EventMeta {
                origin: self.origin,
                iteration: self.iteration,
                seq_no: self.next_seq_no,
            },
            payload,
        };
        self.next_seq_no += 1;
        event
    }

    fn next_iteration(&mut self) {
        self.iteration += 1;
        self.next_seq_no = 0;
    }
}

type OnRuntimeEventFn<E> = dyn Fn(&RuntimeEvent<E>) + 'static;

pub struct FrameworkEventsManager<E> {
    state: EventRuntimeState,
    on_event: Vec<Box<OnRuntimeEventFn<E>>>,
}

impl<E> std::fmt::Debug for FrameworkEventsManager<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FrameworkEventsManager {{ state: {:?}, on_any: {} }}",
            self.state,
            self.on_event.len()
        )
    }
}

impl<E> FrameworkEventsManager<E> {
    pub fn new(origin: EventOrigin, iteration: u32) -> Self {
        Self {
            state: EventRuntimeState::new(origin, iteration),
            on_event: Vec::new(),
        }
    }

    pub fn process_event(&mut self, payload: E) -> RuntimeEvent<E> {
        let event = self.state.wrap(payload);
        for callback in &self.on_event {
            callback(&event);
        }
        event
    }

    pub fn next_iteration(&mut self) {
        self.state.next_iteration();
    }

    pub fn on_event<F>(&mut self, callback: F)
    where
        F: Fn(&RuntimeEvent<E>) + 'static,
    {
        self.on_event.push(Box::new(callback));
    }
}

impl FrameworkEventsManager<MobsimEvent> {
    pub fn for_partition(qsim_id: QSimId, iteration: u32) -> Self {
        Self::new(EventOrigin::Partition(qsim_id), iteration)
    }
}

impl Default for FrameworkEventsManager<MobsimEvent> {
    fn default() -> Self {
        Self::for_partition(0, 0)
    }
}

impl FrameworkEventsManager<ControllerEvent> {
    pub fn for_controller(iteration: u32) -> Self {
        Self::new(EventOrigin::Controller, iteration)
    }
}

impl Default for FrameworkEventsManager<ControllerEvent> {
    fn default() -> Self {
        Self::for_controller(0)
    }
}
