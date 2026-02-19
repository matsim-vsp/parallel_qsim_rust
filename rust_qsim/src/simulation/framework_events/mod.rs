pub type QSimId = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MobsimEvent {
    Initialized,
    BeforeSimStep(MobsimTimeEvent),
    AfterSimStep(MobsimTimeEvent),
    BeforeCleanup,
}

impl MobsimEvent {
    pub fn before_sim_step(time: u32) -> Self {
        Self::BeforeSimStep(MobsimTimeEvent { time })
    }

    pub fn after_sim_step(time: u32) -> Self {
        Self::AfterSimStep(MobsimTimeEvent { time })
    }
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

impl ControllerEvent {
    fn with_flag(last_iteration: bool) -> GeneralControllerEvent {
        GeneralControllerEvent { last_iteration }
    }

    pub fn startup(last_iteration: bool) -> Self {
        Self::Startup(Self::with_flag(last_iteration))
    }

    pub fn iteration_starts(last_iteration: bool) -> Self {
        Self::IterationStarts(Self::with_flag(last_iteration))
    }

    pub fn before_mobsim(last_iteration: bool) -> Self {
        Self::BeforeMobsim(Self::with_flag(last_iteration))
    }

    pub fn after_mobsim(last_iteration: bool) -> Self {
        Self::AfterMobsim(Self::with_flag(last_iteration))
    }

    pub fn scoring(last_iteration: bool) -> Self {
        Self::Scoring(Self::with_flag(last_iteration))
    }

    pub fn replanning(last_iteration: bool) -> Self {
        Self::Replanning(Self::with_flag(last_iteration))
    }

    pub fn iteration_ends(last_iteration: bool) -> Self {
        Self::IterationEnds(Self::with_flag(last_iteration))
    }

    pub fn shutdown(last_iteration: bool) -> Self {
        Self::Shutdown(Self::with_flag(last_iteration))
    }
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

pub type MobsimListenerRegisterFn = dyn FnOnce(&mut MobsimEventsManager) + Send;
pub type ControllerListenerRegisterFn = dyn FnOnce(&mut ControllerEventsManager) + Send;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn controller_events_invoke_callbacks_with_controller_meta() {
        let mut manager = ControllerEventsManager::for_controller(3);
        let received: Rc<RefCell<Vec<ControllerRuntimeEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let received_clone = received.clone();

        manager.on_event(move |event| {
            received_clone.borrow_mut().push(event.clone());
        });

        manager.process_event(ControllerEvent::startup(false));
        manager.process_event(ControllerEvent::iteration_starts(true));

        let events = received.borrow();
        assert_eq!(2, events.len());

        assert_eq!(EventOrigin::Controller, events[0].meta.origin);
        assert_eq!(3, events[0].meta.iteration);
        assert_eq!(0, events[0].meta.seq_no);
        assert_eq!(ControllerEvent::startup(false), events[0].payload.clone());

        assert_eq!(EventOrigin::Controller, events[1].meta.origin);
        assert_eq!(3, events[1].meta.iteration);
        assert_eq!(1, events[1].meta.seq_no);
        assert_eq!(
            ControllerEvent::iteration_starts(true),
            events[1].payload.clone()
        );
    }

    #[test]
    fn controller_next_iteration_resets_seq_counter() {
        let mut manager = ControllerEventsManager::for_controller(10);

        let first = manager.process_event(ControllerEvent::startup(false));
        assert_eq!(10, first.meta.iteration);
        assert_eq!(0, first.meta.seq_no);

        manager.next_iteration();

        let second = manager.process_event(ControllerEvent::shutdown(true));
        assert_eq!(11, second.meta.iteration);
        assert_eq!(0, second.meta.seq_no);
    }

    #[test]
    fn mobsim_events_use_partition_origin_and_invoke_callbacks() {
        let mut manager = MobsimEventsManager::for_partition(7, 2);
        let received: Rc<RefCell<Vec<MobsimRuntimeEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let received_clone = received.clone();

        manager.on_event(move |event| {
            received_clone.borrow_mut().push(event.clone());
        });

        manager.process_event(MobsimEvent::before_sim_step(100));
        manager.process_event(MobsimEvent::after_sim_step(100));
        manager.process_event(MobsimEvent::BeforeCleanup);

        let events = received.borrow();
        assert_eq!(3, events.len());

        assert_eq!(EventOrigin::Partition(7), events[0].meta.origin);
        assert_eq!(2, events[0].meta.iteration);
        assert_eq!(0, events[0].meta.seq_no);
        assert_eq!(MobsimEvent::before_sim_step(100), events[0].payload.clone());

        assert_eq!(EventOrigin::Partition(7), events[1].meta.origin);
        assert_eq!(2, events[1].meta.iteration);
        assert_eq!(1, events[1].meta.seq_no);
        assert_eq!(MobsimEvent::after_sim_step(100), events[1].payload.clone());

        assert_eq!(EventOrigin::Partition(7), events[2].meta.origin);
        assert_eq!(2, events[2].meta.iteration);
        assert_eq!(2, events[2].meta.seq_no);
        assert_eq!(MobsimEvent::BeforeCleanup, events[2].payload.clone());
    }

    #[test]
    fn mobsim_next_iteration_resets_seq_counter() {
        let mut manager = MobsimEventsManager::for_partition(1, 5);

        let first = manager.process_event(MobsimEvent::before_sim_step(1));
        assert_eq!(5, first.meta.iteration);
        assert_eq!(0, first.meta.seq_no);

        manager.next_iteration();

        let second = manager.process_event(MobsimEvent::after_sim_step(2));
        assert_eq!(6, second.meta.iteration);
        assert_eq!(0, second.meta.seq_no);
    }
}
