use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Cursor;

use crate::simulation::id::Id;
use crate::simulation::time_queue::{EndTime, Identifiable};
use crate::simulation::vehicles::io::IOVehicle;
use crate::simulation::wire_types::general::AttributeValue;
use crate::simulation::wire_types::messages::sim_message::Type;
use crate::simulation::wire_types::messages::{
    Empty, PlanLogic, RollingHorizonLogic, SimMessage, SimulationAgent, SimulationAgentLogic,
    StorageCap, SyncMessage, TravelTimesMessage, Vehicle,
};
use crate::simulation::wire_types::population::{Activity, Leg, Person};
use crate::simulation::wire_types::vehicles::VehicleType;
use prost::Message;

impl SimMessage {
    pub fn sync_message(self) -> SyncMessage {
        match self.r#type.unwrap() {
            Type::Sync(m) => m,
            _ => panic!("That message is no sync message."),
        }
    }

    pub fn travel_times_message(self) -> TravelTimesMessage {
        match self.r#type.unwrap() {
            Type::TravelTimes(t) => t,
            _ => panic!("That message is no travel times message."),
        }
    }

    pub fn from_sync_message(m: SyncMessage) -> SimMessage {
        SimMessage {
            r#type: Some(Type::Sync(m)),
        }
    }

    pub fn from_travel_times_message(m: TravelTimesMessage) -> SimMessage {
        SimMessage {
            r#type: Some(Type::TravelTimes(m)),
        }
    }

    pub fn from_empty(m: Empty) -> SimMessage {
        SimMessage {
            r#type: Some(Type::Barrier(m)),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buffer).unwrap();
        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> SimMessage {
        SimMessage::decode(&mut Cursor::new(buffer)).unwrap()
    }
}

impl SyncMessage {
    pub fn new(time: u32, from: u32, to: u32) -> Self {
        Self {
            time,
            from_process: from,
            to_process: to,
            vehicles: Vec::new(),
            storage_capacities: Vec::new(),
        }
    }

    pub fn add_veh(&mut self, vehicle: Vehicle) {
        self.vehicles.push(vehicle);
    }

    pub fn add_storage_cap(&mut self, storage_cap: StorageCap) {
        self.storage_capacities.push(storage_cap);
    }
}

impl TravelTimesMessage {
    pub fn new() -> Self {
        TravelTimesMessage {
            travel_times_by_link_id: HashMap::new(),
        }
    }

    pub fn from(map: HashMap<u64, u32>) -> Self {
        TravelTimesMessage {
            travel_times_by_link_id: map,
        }
    }

    pub fn add_travel_time(&mut self, link: u64, travel_time: u32) {
        self.travel_times_by_link_id.insert(link, travel_time);
    }
}

// Implementation for ordering, so that vehicle messages can be put into a message queue sorted by time
impl PartialOrd for SyncMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for SyncMessage {}

impl Ord for SyncMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.cmp(&self.time)
    }
}

impl Vehicle {
    // todo, fix type and mode
    pub fn new(
        id: u64,
        veh_type: u64,
        max_v: f32,
        pce: f32,
        driver: Option<SimulationAgent>,
    ) -> Vehicle {
        Vehicle {
            id,
            driver,
            curr_route_elem: 0,
            r#type: veh_type,
            max_v,
            pce,
            passengers: vec![],
            attributes: Default::default(),
        }
    }

    pub fn from_io(io_veh: IOVehicle, veh_type: &VehicleType) -> Vehicle {
        let veh_id = Id::<Vehicle>::create(&io_veh.id);
        let veh_type_id = Id::<VehicleType>::get_from_ext(&io_veh.vehicle_type);

        let mut attributes = HashMap::new();
        if let Some(attr) = io_veh.attributes {
            for x in attr.attributes {
                let key = x.name.clone();
                let value = AttributeValue::from_io_attr(x);
                attributes.insert(key, value);
            }
        }

        Vehicle {
            id: veh_id.internal(),
            curr_route_elem: 0,
            r#type: veh_type_id.internal(),
            max_v: veh_type.max_v,
            pce: veh_type.pce,
            driver: None,
            passengers: vec![],
            attributes,
        }
    }

    pub fn driver(&self) -> &SimulationAgent {
        self.driver.as_ref().unwrap()
    }

    pub fn passengers(&self) -> &Vec<SimulationAgent> {
        &self.passengers
    }

    pub fn id(&self) -> usize {
        self.id as usize
    }

    pub fn register_moved_to_next_link(&mut self) {
        self.curr_route_elem += 1;
    }

    pub fn register_vehicle_exited(&mut self) {
        self.curr_route_elem = 0;
    }

    /// This method advances the pointer to the last element of the route. We need this in case of
    /// teleported legs. Advancing the route pointer to the last element directly ensures that teleporting
    /// the vehicle is independent of whether the leg has a Generic-Teleportation route or a network
    /// route.
    pub fn route_index_to_last(&mut self) {
        let route_len = self.driver().curr_leg().route.as_ref().unwrap().route.len() as u32;
        self.curr_route_elem = route_len - 1;
    }

    pub fn curr_link_id(&self) -> Option<u64> {
        let leg = self.driver().curr_leg();
        let route = leg.route.as_ref().unwrap();
        let index = self.curr_route_elem as usize;
        route.route.get(index).copied()
    }

    // todo same as above
    pub fn is_current_link_last(&self) -> bool {
        let leg = self.driver().curr_leg();
        let route = leg.route.as_ref().unwrap();
        self.curr_route_elem + 1 >= route.route.len() as u32
    }

    pub fn peek_next_route_element(&self) -> Option<u64> {
        let route = self.driver().curr_leg().route.as_ref().unwrap();
        let next_i = self.curr_route_elem as usize + 1;
        route.route.get(next_i).copied()
    }
}

impl EndTime for Vehicle {
    fn end_time(&self, now: u32) -> u32 {
        self.driver().end_time(now)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SimulationAgentState {
    LEG,
    ACTIVITY,
    STUCK,
}

impl SimulationAgent {
    pub fn new_plan_logic(person: Person) -> Self {
        let agent_logic = Some(SimulationAgentLogic::new_plan_logic(person));
        SimulationAgent { agent_logic }
    }

    pub fn id(&self) -> u64 {
        self.agent_logic.as_ref().expect("No AgentLogic").id()
    }

    pub fn curr_act(&self) -> &Activity {
        self.agent_logic.as_ref().expect("No AgentLogic").curr_act()
    }

    pub fn curr_leg(&self) -> &Leg {
        self.agent_logic.as_ref().expect("No AgentLogic").curr_leg()
    }

    pub fn advance_plan(&mut self) {
        self.agent_logic
            .as_mut()
            .expect("No AgentLogic")
            .advance_plan()
    }

    pub fn state(&self) -> SimulationAgentState {
        self.agent_logic.as_ref().unwrap().state()
    }

    // we want this in principle, but therefore the curr_route_elem has to be moved from vehicle to agent logic
    // fn choose_next_link_id(&mut self) -> u64;
}

impl EndTime for SimulationAgent {
    fn end_time(&self, now: u32) -> u32 {
        self.agent_logic.as_ref().unwrap().end_time(now)
    }
}

impl Identifiable for SimulationAgent {
    fn id(&self) -> u64 {
        self.agent_logic.as_ref().unwrap().id()
    }
}

impl SimulationAgentLogic {
    pub fn new_plan_logic(person: Person) -> Self {
        SimulationAgentLogic {
            r#type: Some(
                crate::simulation::wire_types::messages::simulation_agent_logic::Type::PlanLogic(
                    PlanLogic {
                        person: Some(person),
                    },
                ),
            ),
        }
    }

    pub fn new_rolling_logic(_person: Person) -> Self {
        // SimulationAgentLogic {
        //     r#type: Some(
        //         crate::simulation::wire_types::messages::simulation_agent_logic::Type::RollingLogic(
        //             RollingLogic {
        //                 person: Some(person),
        //             },
        //         ),
        //     ),
        // }
        unimplemented!("Rolling logic not implemented yet")
    }

    pub fn id(&self) -> u64 {
        match self.r#type.as_ref().unwrap() {
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::PlanLogic(l) => {
                l.id()
            }
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::RollingHorizonLogic(
                l,
            ) => l.id(),
        }
    }

    pub fn curr_act(&self) -> &Activity {
        match self.r#type.as_ref().unwrap() {
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::PlanLogic(l) => {
                l.curr_act()
            }
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::RollingHorizonLogic(
                l,
            ) => l.curr_act(),
        }
    }

    pub fn curr_leg(&self) -> &Leg {
        match self.r#type.as_ref().unwrap() {
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::PlanLogic(l) => {
                l.curr_leg()
            }
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::RollingHorizonLogic(
                l,
            ) => l.curr_leg(),
        }
    }

    pub fn advance_plan(&mut self) {
        match self.r#type.as_mut().unwrap() {
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::PlanLogic(l) => {
                l.advance_plan();
            }
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::RollingHorizonLogic(
                l,
            ) => l.advance_plan(),
        }
    }

    pub fn end_time(&self, now: u32) -> u32 {
        match self.r#type.as_ref().unwrap() {
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::PlanLogic(l) => {
                l.person.as_ref().unwrap().end_time(now)
            }
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::RollingHorizonLogic(
                l,
            ) => l.person.as_ref().unwrap().end_time(now),
        }
    }

    pub fn state(&self) -> SimulationAgentState {
        match self.r#type.as_ref().unwrap() {
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::PlanLogic(l) => {
                if l.person.as_ref().unwrap().curr_plan_elem % 2 == 0 {
                    SimulationAgentState::ACTIVITY
                } else {
                    SimulationAgentState::LEG
                }
            }
            crate::simulation::wire_types::messages::simulation_agent_logic::Type::RollingHorizonLogic(
                _l,
            ) => unimplemented!(),
        }
    }
}

impl PlanLogic {
    pub fn id(&self) -> u64 {
        self.person.as_ref().unwrap().id
    }

    pub fn curr_act(&self) -> &Activity {
        self.person.as_ref().unwrap().curr_act()
    }

    pub fn curr_leg(&self) -> &Leg {
        self.person.as_ref().unwrap().curr_leg()
    }

    pub fn advance_plan(&mut self) {
        self.person.as_mut().unwrap().advance_plan();
    }

    pub fn end_time(&self, now: u32) -> u32 {
        let person = self.person.as_ref().unwrap();

        if person.curr_plan_elem % 2 == 0 {
            person.curr_act().cmp_end_time(now)
        } else {
            self.curr_leg().trav_time + now
        }
    }

    pub fn state(&self) -> SimulationAgentState {
        if self.person.as_ref().unwrap().curr_plan_elem % 2 == 0 {
            SimulationAgentState::ACTIVITY
        } else {
            SimulationAgentState::LEG
        }
    }
}

impl RollingHorizonLogic {
    pub fn id(&self) -> u64 {
        unimplemented!()
    }

    pub fn curr_act(&self) -> &Activity {
        unimplemented!()
    }

    pub fn curr_leg(&self) -> &Leg {
        unimplemented!()
    }

    pub fn advance_plan(&mut self) {
        unimplemented!()
    }

    pub fn state(&self) -> SimulationAgentState {
        unimplemented!()
    }
}
