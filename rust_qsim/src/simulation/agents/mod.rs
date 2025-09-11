pub mod agent;
pub mod agent_logic;

use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::id::Id;
use crate::simulation::network::Link;
use crate::simulation::population::{InternalActivity, InternalLeg, InternalPerson};
use crate::simulation::time_queue::{EndTime, Identifiable};
use std::fmt::Debug;

pub trait SimulationAgentLogic:
    EndTime + Identifiable<InternalPerson> + EnvironmentalEventObserver + Send
{
    fn curr_act(&self) -> &InternalActivity;
    fn next_act(&self) -> &InternalActivity;
    fn curr_leg(&self) -> &InternalLeg;
    fn next_leg(&self) -> Option<&InternalLeg>;
    fn advance_plan(&mut self);
    fn wakeup_time(&self, now: u32) -> u32;
    fn state(&self) -> SimulationAgentState;
    fn curr_link_id(&self) -> Option<&Id<Link>>;
    fn peek_next_link_id(&self) -> Option<&Id<Link>>;
}

pub trait EnvironmentalEventObserver {
    fn notify_event(&mut self, event: &mut AgentEvent, now: u32);
}

#[non_exhaustive]
pub enum AgentEvent<'a> {
    ActivityStarted(),
    Wakeup(WakeupEvent<'a>),
    ActivityFinished(),
    TeleportationStarted(),
    TeleportationFinished(),
    NetworkLegStarted(),
    MovedToNextLink(),
    NetworkLegFinished(),
}

pub struct WakeupEvent<'w> {
    pub comp_env: &'w mut ThreadLocalComputationalEnvironment,
    pub end_time: u32,
}

impl Debug for dyn SimulationAgentLogic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Simulation Agent Logic for agent with id {}",
            self.id().external()
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SimulationAgentState {
    LEG,
    ACTIVITY,
    STUCK,
}
