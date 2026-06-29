use std::any::{Any, TypeId};
use std::cmp::{Ordering};
use std::collections::{BinaryHeap, HashMap};
use std::fmt::{Display, Formatter};
use crate::generated::population::Activity;
use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventTrait, LinkEnterEvent, PersonArrivalEvent, PersonDepartureEvent, PersonEntersVehicleEvent, PersonLeavesVehicleEvent, TeleportationArrivalEvent, VehicleEntersTrafficEvent, VehicleLeavesTrafficEvent};


/// A special buffer, designed to solve the 2-hop-termination problem. It uses a state machine to
/// ensure that events are returned in the correct order. The state machine is designed with a sparse
/// transition table in mind. Non-defined transitions will simply panic.
///
/// The process_event function takes a new event, which updates the state and returns the events in
/// ordered state as a vector. In most cases, the buffer will return a 1-element vector of the passed event.
/// If the buffer enters a state where it cannot ensure correct ordering of events, it will return
/// empty vectors unless a safe state is reached where all events are returned in one list.
///
/// Internally, events are ordered using a (t, n) tuple, where t is the global simulation time of
/// the event and n is a partition-internal iterating counter.
struct StateMachineBuffer {
    n: u16,
    state_name2state: HashMap<&'static str, State>,
    transitions: HashMap<(State, TypeId), State>,

    current_state: State,
    buffer: BinaryHeap<Entry>,
}

impl StateMachineBuffer {
    pub fn default_state_machine_buffer() -> StateMachineBuffer {
        let states = vec![
            "Departure",
            "D.1",
            "D.2",
            "D.3",
            "Arrival",
            "A.1",
            "A.2"
        ];
        StateMachineBufferBuilder::new(states)
            .add_transition("safe", TypeId::of::<ActivityEndEvent>(), "Departure")
            .add_transition("safe", TypeId::of::<VehicleLeavesTrafficEvent>(), "Arrival")
            .add_transition("safe", TypeId::of::<PersonLeavesVehicleEvent>(), "Arrival")
            .add_transition("safe", TypeId::of::<PersonArrivalEvent>(), "Arrival")
            .add_transition("safe", TypeId::of::<ActivityStartEvent>(), "Arrival")

            .add_transition("Departure", TypeId::of::<PersonDepartureEvent>(), "D.1")

            .add_transition("D.1", TypeId::of::<PersonEntersVehicleEvent>(), "D.2")
            .add_transition("D.1", TypeId::of::<VehicleLeavesTrafficEvent>(), "D.2")
            .add_transition("D.1", TypeId::of::<TeleportationArrivalEvent>(), "safe")

            .add_transition("D.2", TypeId::of::<PersonEntersVehicleEvent>(), "D.3")
            .add_transition("D.2", TypeId::of::<VehicleEntersTrafficEvent>(), "D.3")

            .add_transition("D.3", TypeId::of::<LinkEnterEvent>(), "safe")
            .add_transition("D.3", TypeId::of::<VehicleLeavesTrafficEvent>(), "Arrival")
            .add_transition("D.3", TypeId::of::<PersonLeavesVehicleEvent>(), "Arrival")
            .add_transition("D.3", TypeId::of::<PersonArrivalEvent>(), "Arrival")
            .add_transition("D.3", TypeId::of::<ActivityStartEvent>(), "Arrival")

            .add_transition("Arrival", TypeId::of::<VehicleLeavesTrafficEvent>(), "A.1")
            .add_transition("Arrival", TypeId::of::<PersonLeavesVehicleEvent>(), "A.1")
            .add_transition("Arrival", TypeId::of::<PersonArrivalEvent>(), "A.1")
            .add_transition("Arrival", TypeId::of::<ActivityStartEvent>(), "A.1")

            .add_transition("A.1", TypeId::of::<VehicleLeavesTrafficEvent>(), "A.2")
            .add_transition("A.1", TypeId::of::<PersonLeavesVehicleEvent>(), "A.2")
            .add_transition("A.1", TypeId::of::<PersonArrivalEvent>(), "A.2")
            .add_transition("A.1", TypeId::of::<ActivityStartEvent>(), "A.2")

            .add_transition("A.2", TypeId::of::<VehicleLeavesTrafficEvent>(), "safe")
            .add_transition("A.2", TypeId::of::<PersonLeavesVehicleEvent>(), "safe")
            .add_transition("A.2", TypeId::of::<PersonArrivalEvent>(), "safe")
            .add_transition("A.2", TypeId::of::<ActivityStartEvent>(), "safe")

            .build()
    }

    fn new(n: u16, state_name2state: HashMap<&'static str, State>, transitions: HashMap<(State, TypeId), State>) -> Self {
        let starting_state = *state_name2state.get("safe").unwrap();
        Self { n, state_name2state, transitions, current_state: starting_state, buffer: BinaryHeap::new() }
    }

    pub fn process_event(&mut self, sorting_key: u32, event: Box<dyn EventTrait>) -> Vec<Box<dyn EventTrait>> {
        let new_state = self.transitions.get(&(self.current_state, event.type_id())).unwrap_or_else(
            || panic!("Entered undefined transition! ({}, {})", self.current_state, event.type_())
        );

        // If the new state is emitting, then retrieve buffer and append current event
        if new_state.1 {
            self.current_state = *new_state;

            // For efficiency: Check if buffer is empty
            if self.buffer.is_empty() {
                // If buffer is empty: Skip it entirely -> Reduced technical overhead
                return vec![event];
            }

            // Buffer contains elements, make sure that arriving event is ordered
            self.buffer.push(Entry(sorting_key, event));
            let sorted = std::mem::take(&mut self.buffer).into_sorted_vec().into_iter().map(|e| e.1).collect();

            return sorted;
        }

        // New state is not emitting: Buffer the arriving event and return an empty vec
        self.buffer.push(Entry(sorting_key, event));

        vec![]
    }
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
struct State(u16, bool);

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

struct Entry(u32, Box<dyn EventTrait>);

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
impl Eq for Entry {}

struct StateMachineBufferBuilder {
    // Attributes for the state machine
    n: u16,
    state_name2state: HashMap<&'static str, State>,
    transitions: HashMap<(State, TypeId), State>,

    // Buffer
    buffer: Vec<Box<dyn EventTrait>>,
}

impl StateMachineBufferBuilder {
    pub fn new(mut states: Vec<&'static str>) -> Self {
        states.push("safe");
        let state_name2state = states.into_iter().enumerate().map(|(i, name)| (name, State(i as u16, false))).collect();

        Self {
            n: 0,
            state_name2state,
            transitions: HashMap::default(),
            buffer: Vec::new(),
        }
    }

    pub fn add_state(mut self, name: &'static str, emit: bool) -> Self {
        let s = State(self.n, emit);
        self.state_name2state.insert(name, s);
        self.n += 1;

        self
    }

    pub fn add_transition(mut self, state_name: &str, type_id: TypeId, new_state_name: &str) -> Self {
        let state = *self.state_name2state.get(state_name).unwrap_or_else(
            || panic!("State {} is not defined!", state_name)
        );

        if let Some(colliding_new_state) = self.transitions.get(&(state, type_id)) {
            panic!("Tried to add a transition that already exists! Input state: {}, Input event: {:?}, Colliding transitions: {}, {}", state_name, type_id, new_state_name, colliding_new_state);
        }

        let new_state = *self.state_name2state.get(new_state_name).unwrap_or_else(
            || panic!("State {} is not defined!", new_state_name)
        );

        self.transitions.insert((state, type_id.type_id()), new_state);

        self
    }


    pub fn build(self) -> StateMachineBuffer {
        StateMachineBuffer::new(self.n, self.state_name2state, self.transitions)
    }
}