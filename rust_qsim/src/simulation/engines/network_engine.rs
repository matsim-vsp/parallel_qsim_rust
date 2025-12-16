use tracing::instrument;
use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::vehicles::InternalVehicle;

pub struct NetworkEngine {
    pub(crate) network: SimNetworkPartition,
    comp_env: ThreadLocalComputationalEnvironment,
}

impl NetworkEngine {
    pub fn new(
        network: SimNetworkPartition,
        comp_env: ThreadLocalComputationalEnvironment,
    ) -> Self {
        NetworkEngine { network, comp_env }
    }

    pub fn receive_vehicle(&mut self, now: u32, vehicle: InternalVehicle, route_begin: bool) {
        let events = if route_begin {
            //if route has just begun, no link enter event should be published
            None
        } else {
            //if route is already in progress, this method gets vehicles from another partition and should publish link enter event
            //this is because the receiving partition is the owner of this link and should publish the event
            Some(self.comp_env.events_publisher())
        };
        self.network.send_veh_en_route(vehicle, events, now)
    }

    #[instrument(level = "trace", skip(self), fields(rank = self.network.partition()))]
    pub(super) fn move_nodes(&mut self, now: u32) {
        self.network.move_nodes(&mut self.comp_env, now)
    }

    #[instrument(level = "trace", skip(self, net_message_broker), fields(rank = self.network.partition()))]
    pub(super) fn move_links<C: SimCommunicator>(
        &mut self,
        now: u32,
        net_message_broker: &mut NetMessageBroker<C>,
    ) -> Vec<InternalVehicle> {
        let move_links_result = self.network.move_links(&mut self.comp_env, now);

        for veh in move_links_result.vehicles_exit_partition {
            net_message_broker.add_veh(veh, now);
        }

        for cap in move_links_result.storage_cap_updates {
            net_message_broker.add_cap_update(cap, now);
        }

        move_links_result.vehicles_end_leg
    }
}
