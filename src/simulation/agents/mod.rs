pub mod agent;
pub mod agent_logic;

use crate::simulation::controller::local_controller::ComputationalEnvironment;
use crate::simulation::id::Id;
use crate::simulation::network::Link;
use crate::simulation::population::{InternalActivity, InternalLeg, InternalPerson};
use crate::simulation::time_queue::{EndTime, Identifiable};
use std::fmt::Debug;

pub trait SimulationAgentLogic:
    EndTime + Identifiable<InternalPerson> + EnvironmentalEventObserver + Send
{
    fn curr_act(&self) -> &InternalActivity;
    fn curr_leg(&self) -> &InternalLeg;
    fn next_leg(&self) -> Option<&InternalLeg>;
    fn advance_plan(&mut self);
    fn wakeup_time(&self, now: u32) -> u32;
    fn state(&self) -> SimulationAgentState;
    fn curr_link_id(&self) -> Option<&Id<Link>>;
    fn peek_next_link_id(&self) -> Option<&Id<Link>>;
}

pub trait EnvironmentalEventObserver {
    fn notify_event(&mut self, event: AgentEvent, now: u32);
}

#[non_exhaustive]
#[derive(Clone)]
pub enum AgentEvent {
    ActivityStarted { comp_env: ComputationalEnvironment },
    Wakeup { comp_env: ComputationalEnvironment },
    ActivityFinished { comp_env: ComputationalEnvironment },
    TeleportationStarted { comp_env: ComputationalEnvironment },
    TeleportationFinished { comp_env: ComputationalEnvironment },
    NetworkLegStarted { comp_env: ComputationalEnvironment },
    MovedToNextLink { comp_env: ComputationalEnvironment },
    NetworkLegFinished { comp_env: ComputationalEnvironment },
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
