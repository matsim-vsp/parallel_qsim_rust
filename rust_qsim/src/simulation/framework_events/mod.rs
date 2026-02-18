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
