use std::marker::PhantomData;

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
    pub iteration: u32,
    pub last_iteration: u32,
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

type OnRuntimeEventFn<E> = dyn Fn(&RuntimeEvent<E>) + 'static;

pub type MobsimListenerRegistrator = dyn FnOnce(&mut MobsimEventBus) + Send;
pub type ControllerListenerRegistrator = dyn FnOnce(&mut ControllerEventBus) + Send;

pub struct FrameworkEventBus<E> {
    origin: EventOrigin,
    iteration: u32,
    next_seq_no: u64,
    callbacks: Vec<Box<OnRuntimeEventFn<E>>>,
    _event_type: PhantomData<E>,
}

impl<E> std::fmt::Debug for FrameworkEventBus<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EventBus {{ origin: {:?}, iteration: {}, next_seq_no: {}, callbacks: {} }}",
            self.origin,
            self.iteration,
            self.next_seq_no,
            self.callbacks.len()
        )
    }
}

impl<E> FrameworkEventBus<E> {
    pub fn new(origin: EventOrigin, iteration: u32) -> Self {
        Self {
            origin,
            iteration,
            next_seq_no: 0,
            callbacks: Vec::new(),
            _event_type: PhantomData,
        }
    }

    pub fn process(&mut self, payload: E) -> RuntimeEvent<E> {
        let event = RuntimeEvent {
            meta: EventMeta {
                origin: self.origin,
                iteration: self.iteration,
                seq_no: self.next_seq_no,
            },
            payload,
        };
        for callback in &self.callbacks {
            callback(&event);
        }
        self.next_seq_no += 1;
        event
    }

    pub fn set_iteration(&mut self, iteration: u32) {
        self.iteration = iteration;
        self.next_seq_no = 0;
    }

    pub fn on_event<F>(&mut self, callback: F)
    where
        F: Fn(&RuntimeEvent<E>) + 'static,
    {
        self.callbacks.push(Box::new(callback));
    }
}

impl FrameworkEventBus<MobsimEvent> {
    pub fn for_partition(qsim_id: QSimId, iteration: u32) -> Self {
        Self::new(EventOrigin::Partition(qsim_id), iteration)
    }

    pub fn on_before_sim_step<F>(&mut self, callback: F)
    where
        F: Fn(&MobsimRuntimeEvent) + 'static,
    {
        self.on_event(move |event| {
            if let MobsimEvent::BeforeSimStep(_) = &event.payload {
                callback(event);
            }
        });
    }
}

impl FrameworkEventBus<ControllerEvent> {
    pub fn for_controller(iteration: u32) -> Self {
        Self::new(EventOrigin::Controller, iteration)
    }

    pub fn on_startup<F>(&mut self, callback: F)
    where
        F: Fn(&ControllerRuntimeEvent) + 'static,
    {
        self.on_event(move |event| {
            if let ControllerEvent::Startup(_) = &event.payload {
                callback(event);
            }
        });
    }
}

impl Default for FrameworkEventBus<MobsimEvent> {
    fn default() -> Self {
        Self::for_partition(0, 0)
    }
}

impl Default for FrameworkEventBus<ControllerEvent> {
    fn default() -> Self {
        Self::for_controller(0)
    }
}

pub type MobsimEventBus = FrameworkEventBus<MobsimEvent>;
pub type ControllerEventBus = FrameworkEventBus<ControllerEvent>;
