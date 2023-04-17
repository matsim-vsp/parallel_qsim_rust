use crate::simulation::performance_profiling::proto::metadata::Type::{
    NodeInformation, QSimStep, QSimUpdate, TravelTimeUpdate,
};
use crate::simulation::performance_profiling::proto::{
    Metadata, NodeInformationData, ProfilingEvent, QSimStepData, QSimUpdateData, SimulationProfile,
    TravelTimeUpdateData,
};

impl ProfilingEvent {
    pub fn new(key: String, now: Option<u32>, duration: u64, metadata: Option<Metadata>) -> Self {
        Self {
            key,
            duration,
            sim_time: now,
            metadata,
        }
    }
}

impl Metadata {
    pub fn new_node_information(
        local_links: u64,
        split_in_links: u64,
        split_out_links: u64,
        neighbours: Vec<u64>,
    ) -> Self {
        Metadata {
            r#type: Some(NodeInformation(NodeInformationData {
                local_links,
                split_in_links,
                split_out_links,
                neighbours,
            })),
        }
    }

    pub fn new_qsim_step(vehicles: u64, agents: u64) -> Self {
        Metadata {
            r#type: Some(QSimStep(QSimStepData { vehicles, agents })),
        }
    }

    pub fn new_qsim_update(size: u64) -> Self {
        Metadata {
            r#type: Some(QSimUpdate(QSimUpdateData { size })),
        }
    }

    pub fn new_travel_time_collecting(updates: u64) -> Self {
        Metadata {
            r#type: Some(TravelTimeUpdate(TravelTimeUpdateData { updates })),
        }
    }
}

impl SimulationProfile {
    pub fn new() -> Self {
        Self {
            profiling_events: Vec::new(),
        }
    }

    pub fn add_profiling_event(&mut self, event: ProfilingEvent) {
        self.profiling_events.push(event);
    }
}
