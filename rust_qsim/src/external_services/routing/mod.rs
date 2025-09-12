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
use tracing::info;

pub struct RoutingServiceAdapter {
    client: Vec<RoutingServiceClient<tonic::transport::Channel>>,
    counter: usize,
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

/// Factory for creating routing service adapters. Connects to the routing service at the given IP address.
pub struct RoutingServiceAdapterFactory {
    ip: Vec<String>,
    //TODO think about whether this should be an Arc<Config> or not
    config: Config,
}

impl RoutingServiceAdapterFactory {
    pub fn new(ip: Vec<impl Into<String>>, config: Config) -> Self {
        Self {
            ip: ip.into_iter().map(|s| s.into()).collect(),
            config,
        }
    }
}

impl RequestAdapterFactory<InternalRoutingRequest> for RoutingServiceAdapterFactory {
    async fn build(self) -> impl RequestAdapter<InternalRoutingRequest> {
        let mut res = Vec::new();
        for ip in self.ip {
            info!("Connecting to routing service at {}", ip);
            let client = RoutingServiceClient::connect(ip)
                .await
                .expect("Failed to connect to routing service");
            res.push(client);
        }
        RoutingServiceAdapter::new(res)
    }

    fn thread_count(&self) -> usize {
        self.config.routing().threads
    }
}

impl RoutingServiceAdapterFactory {
    /// Spawns a thread running a routing service adapter.
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
        let mut client = self.next_client();

        tokio::spawn(async move {
            let request = Request::from(internal_req.payload);

            let response = client
                .get_route(request)
                .await
                .expect("Error while calling routing service");

            let internal_res = InternalRoutingResponse::from(response.into_inner());

            let _ = internal_req.response_tx.send(internal_res);
        });
    }
}

impl RoutingServiceAdapter {
    fn new(client: Vec<RoutingServiceClient<tonic::transport::Channel>>) -> Self {
        Self { client, counter: 0 }
    }

    fn next_client(&mut self) -> RoutingServiceClient<tonic::transport::Channel> {
        let len = self.client.len();
        let client = &mut self.client[self.counter];
        self.counter = (self.counter + 1) % len;
        client.clone()
    }
}
