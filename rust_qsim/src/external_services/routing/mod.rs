use crate::external_services::{
    execute_adapter, RequestAdapter, RequestAdapterFactory, RequestToAdapter,
};
use crate::generated::routing::routing_service_client::RoutingServiceClient;
use crate::generated::routing::{Request, Response};
use crate::simulation::config::Config;
use crate::simulation::population::{InternalActivity, InternalLeg, InternalPlanElement};
use itertools::{EitherOrBoth, Itertools};
use std::thread;
use std::thread::JoinHandle;
use tokio::sync::oneshot::Sender;

pub struct RoutingServiceAdapter {
    client: RoutingServiceClient<tonic::transport::Channel>,
}

#[derive(Debug)]
pub struct InternalRoutingRequest {
    pub payload: InternalRoutingRequestPayload,
    pub response_tx: Sender<InternalRoutingResponse>,
}

impl RequestToAdapter for InternalRoutingRequest {}

#[derive(Debug, PartialEq)]
pub struct InternalRoutingRequestPayload {
    pub person_id: String,
    pub from_link: String,
    pub to_link: String,
    pub mode: String,
    pub departure_time: u32,
    pub now: u32,
}

#[derive(Debug, Clone, Default)]
pub struct InternalRoutingResponse(pub(crate) Vec<InternalPlanElement>);

impl From<InternalRoutingRequestPayload> for Request {
    fn from(req: InternalRoutingRequestPayload) -> Self {
        Request {
            person_id: req.person_id,
            from_link_id: req.from_link,
            to_link_id: req.to_link,
            mode: req.mode,
            departure_time: req.departure_time,
        }
    }
}

impl From<Response> for InternalRoutingResponse {
    fn from(value: Response) -> Self {
        //zip legs and activities
        let legs = value
            .legs
            .into_iter()
            .map(InternalLeg::from)
            .collect::<Vec<_>>();
        let activities = value
            .activities
            .into_iter()
            .map(InternalActivity::from)
            .collect::<Vec<_>>();

        let mut elements = Vec::new();
        for pair in legs.into_iter().zip_longest(activities.into_iter()) {
            match pair {
                EitherOrBoth::Both(l, a) => {
                    elements.push(InternalPlanElement::Leg(l));
                    elements.push(InternalPlanElement::Activity(a));
                }
                EitherOrBoth::Left(l) => {
                    elements.push(InternalPlanElement::Leg(l));
                }
                EitherOrBoth::Right(_) => {
                    panic!("Received routing response ends with an activity, but expected a leg.");
                }
            }
        }

        Self(elements)
    }
}

pub struct RoutingServiceAdapterFactory {
    ip: String,
    //TODO think about whether this should be an Arc<Config> or not
    config: Config,
}

impl RoutingServiceAdapterFactory {
    pub fn new(ip: &str, config: Config) -> Self {
        Self {
            ip: ip.to_string(),
            config,
        }
    }
}

impl RequestAdapterFactory<InternalRoutingRequest> for RoutingServiceAdapterFactory {
    async fn build(self) -> impl RequestAdapter<InternalRoutingRequest> {
        crate::simulation::id::load_from_file(&self.config.proto_files().ids);

        let client = RoutingServiceClient::connect(self.ip)
            .await
            .expect("Failed to connect to routing service");

        RoutingServiceAdapter { client }
    }

    fn thread_count(&self) -> usize {
        self.config.routing().threads
    }
}

impl RoutingServiceAdapterFactory {
    pub fn spawn_thread(
        self,
        name: &str,
    ) -> (
        JoinHandle<()>,
        tokio::sync::mpsc::Sender<InternalRoutingRequest>,
        tokio::sync::watch::Sender<bool>,
    ) {
        let (send, recv) = self.request_channel(10000);
        let (send_sd, recv_sd) = self.shutdown_channel();

        let handle = thread::Builder::new()
            .name(name.into())
            .spawn(move || execute_adapter(recv, self, recv_sd))
            .unwrap();

        (handle, send, send_sd)
    }
}

impl RequestAdapter<InternalRoutingRequest> for RoutingServiceAdapter {
    async fn on_request(&mut self, internal_req: InternalRoutingRequest) {
        let request = Request::from(internal_req.payload);
        let response = self
            .client
            .get_route(request)
            .await
            .expect("Error while calling routing service");

        let internal_res = InternalRoutingResponse::from(response.into_inner());

        let _ = internal_req.response_tx.send(internal_res);
    }
}
