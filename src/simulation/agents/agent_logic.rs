use crate::simulation::agents::{
    AgentEvent, EnvironmentalEventObserver, SimulationAgentLogic, SimulationAgentState,
};
use crate::simulation::id::Id;
use crate::simulation::network::Link;
use crate::simulation::population::{
    InternalActivity, InternalLeg, InternalPerson, InternalPlanElement, InternalRoute,
};
use crate::simulation::time_queue::{EndTime, Identifiable};

#[derive(Debug, PartialEq, Clone)]
pub struct PlanBasedSimulationLogic {
    pub(super) basic_agent_delegate: InternalPerson,
    pub(super) curr_plan_element: usize,
    pub(super) curr_route_element: usize,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AdaptivePlanBasedSimulationLogic {
    delegate: PlanBasedSimulationLogic,
}

impl Identifiable<InternalPerson> for PlanBasedSimulationLogic {
    fn id(&self) -> &Id<InternalPerson> {
        self.basic_agent_delegate.id()
    }
}

impl EnvironmentalEventObserver for PlanBasedSimulationLogic {
    fn notify_event(&mut self, event: AgentEvent, _now: u32) {
        match event {
            AgentEvent::TeleportationStarted { .. } => {
                self.set_curr_route_element_to_last();
            }
            AgentEvent::MovedToNextLink { .. } => {
                self.curr_route_element += 1;
            }
            _ => {}
        }
    }
}

impl PlanBasedSimulationLogic {
    /// This method advances the pointer to the last element of the route. We need this in case of
    /// teleported legs. Advancing the route pointer to the last element directly ensures that teleporting
    /// the vehicle is independent of whether the leg has a Generic-Teleportation route or a network
    /// route.
    fn set_curr_route_element_to_last(&mut self) {
        let route = self.curr_leg().route.as_ref().unwrap();
        if route.as_network().is_some() {
            let last = route.as_network().unwrap().route().len() - 1;
            self.curr_route_element = last;
        } else {
            self.curr_route_element = 1;
        }
    }
}

impl SimulationAgentLogic for PlanBasedSimulationLogic {
    fn curr_act(&self) -> &InternalActivity {
        self.basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
            .as_activity()
            .unwrap()
    }

    fn curr_leg(&self) -> &InternalLeg {
        self.basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
            .as_leg()
            .unwrap()
    }

    fn next_leg(&self) -> Option<&InternalLeg> {
        let add = if self.curr_plan_element % 2 == 0 {
            // If the current plan element is an activity, the next one should be a leg
            1
        } else {
            // If the current plan element is a leg, the next one should be an activity
            2
        };
        self.basic_agent_delegate
            .plan_element_at(self.curr_plan_element + add)
            .as_leg()
    }

    fn advance_plan(&mut self) {
        self.curr_plan_element += 1;
        self.curr_route_element = 0;
        assert!(
            self.curr_plan_element < self.basic_agent_delegate.total_elements(),
            "Cannot advance plan of agents {:?} beyond its last element.",
            self.basic_agent_delegate.id()
        );
    }

    fn wakeup_time(&self, now: u32) -> u32 {
        // TODO this might be adapted with rolling horizon logic

        match self
            .basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
        {
            InternalPlanElement::Activity(a) => a.cmp_end_time(now),
            InternalPlanElement::Leg(_) => panic!("Cannot wake up on a leg!"),
        }
    }

    fn state(&self) -> SimulationAgentState {
        match self.curr_plan_element % 2 {
            0 => SimulationAgentState::ACTIVITY,
            1 => SimulationAgentState::LEG,
            _ => unreachable!(),
        }
    }

    fn curr_link_id(&self) -> Option<&Id<Link>> {
        if self.state() != SimulationAgentState::LEG {
            return None;
        }

        match self.curr_leg().route.as_ref().unwrap() {
            InternalRoute::Generic(g) => match self.curr_route_element {
                0 => Some(g.start_link()),
                1 => Some(g.end_link()),
                _ => panic!(
                    "A generic route only has two elements. Current plan element {:?}, Current route element {:?}, Current agent {:?}", self.curr_plan_element, self.curr_route_element, self.basic_agent_delegate.id()
                ),
            },
            InternalRoute::Network(n) => n.route_element_at(self.curr_route_element),
            InternalRoute::Pt(p) => match self.curr_route_element {
                0 => Some(p.start_link()),
                1 => Some(p.end_link()),
                _ => panic!(
                    "A generic route only has two elements. Current plan element {:?}, Current route element {:?}, Current agent {:?}", self.curr_plan_element, self.curr_route_element, self.basic_agent_delegate.id()
                ),
            },
        }
    }

    fn peek_next_link_id(&self) -> Option<&Id<Link>> {
        let next_i = self.curr_route_element + 1;
        self.curr_leg()
            .route
            .as_ref()
            .unwrap()
            .as_network()
            .unwrap()
            .route_element_at(next_i)
    }
}

impl SimulationAgentLogic for AdaptivePlanBasedSimulationLogic {
    fn curr_act(&self) -> &InternalActivity {
        self.delegate.curr_act()
    }

    fn curr_leg(&self) -> &InternalLeg {
        self.delegate.curr_leg()
    }

    fn next_leg(&self) -> Option<&InternalLeg> {
        self.delegate.next_leg()
    }

    fn advance_plan(&mut self) {
        self.delegate.advance_plan();
    }

    fn wakeup_time(&self, now: u32) -> u32 {
        self.delegate.wakeup_time(now)
    }

    fn state(&self) -> SimulationAgentState {
        self.delegate.state()
    }

    fn curr_link_id(&self) -> Option<&Id<Link>> {
        self.delegate.curr_link_id()
    }

    fn peek_next_link_id(&self) -> Option<&Id<Link>> {
        self.delegate.peek_next_link_id()
    }
}

impl EndTime for AdaptivePlanBasedSimulationLogic {
    fn end_time(&self, now: u32) -> u32 {
        self.delegate.end_time(now)
    }
}

impl Identifiable<InternalPerson> for AdaptivePlanBasedSimulationLogic {
    fn id(&self) -> &Id<InternalPerson> {
        self.delegate.id()
    }
}

impl EnvironmentalEventObserver for AdaptivePlanBasedSimulationLogic {
    fn notify_event(&mut self, event: AgentEvent, now: u32) {
        todo!()
    }
}

impl EndTime for PlanBasedSimulationLogic {
    fn end_time(&self, now: u32) -> u32 {
        match self
            .basic_agent_delegate
            .plan_element_at(self.curr_plan_element)
        {
            InternalPlanElement::Activity(a) => a.cmp_end_time(now),
            InternalPlanElement::Leg(l) => l.trav_time.unwrap() + now,
        }
    }
}
