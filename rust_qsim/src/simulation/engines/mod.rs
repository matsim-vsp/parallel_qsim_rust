use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::framework_events::{
    AgentLeavesPartitionEvent, PartitionEvent, VehicleLeavesPartitionEvent,
};
use crate::simulation::time_queue::Identifiable;
use crate::simulation::vehicles::SimulationVehicle;

pub mod activity_engine;
pub mod leg_engine;
pub mod network_engine;
pub mod teleportation_engine;

fn emit_partition_leave_events(
    comp_env: &mut ThreadLocalComputationalEnvironment,
    vehicle: &SimulationVehicle,
    to: u32,
) {
    comp_env
        .partition_events_manager_borrow_mut()
        .process_event(PartitionEvent::VehicleLeavesPartition(
            VehicleLeavesPartitionEvent {
                vehicle_id: vehicle.id().clone(),
                to,
            },
        ));
    comp_env
        .partition_events_manager_borrow_mut()
        .process_event(PartitionEvent::AgentLeavesPartition(
            AgentLeavesPartitionEvent {
                agent_id: vehicle.driver().id().clone(),
                to,
            },
        ));
}
