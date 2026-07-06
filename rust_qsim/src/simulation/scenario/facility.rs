use crate::simulation::id::Id;
use crate::simulation::scenario::Coordinate;
use crate::simulation::scenario::network::Link;

/// Facility is a location that has modal access to the network.
#[derive(Debug)]
pub enum Facility {
    Link(LinkFacility),
    Activity(ActivityFacility),
    TransitStop(TransitStopFacility),
}

impl Facility {
    pub fn coord(&self) -> Coordinate {
        todo!()
    }

    pub fn modal_link_id(&self, mode: &Id<String>) -> Id<Link> {
        // if there is a mapping from mode to link, return the link id. Otherwise, return the base_link_id.
        todo!()
    }

    pub fn base_link_id(&self) -> Id<Link> {
        todo!()
    }
}

#[derive(Debug)]
pub struct LinkFacility {}

#[derive(Debug)]
pub struct ActivityFacility {}

#[derive(Debug)]
pub struct TransitStopFacility {}
