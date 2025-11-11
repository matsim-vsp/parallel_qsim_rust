use crate::external_services::{RequestAdapter, RequestAdapterFactory, RequestToAdapter};
use crate::generated::routing::routing_service_client::RoutingServiceClient;
use crate::generated::routing::{Request, Response};
use crate::simulation::config::Config;
use crate::simulation::data_structures::RingIter;
use crate::simulation::population::{InternalActivity, InternalLeg, InternalPlanElement};
use derive_builder::Builder;
use itertools::{EitherOrBoth, Itertools};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot::Sender;
use tokio::task::JoinHandle;
use tracing::info;
use uuid::Uuid;

pub struct RoutingServiceAdapter {
    clients: RingIter<RoutingServiceClient<tonic::transport::Channel>>,
    shutdown_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

#[derive(Debug)]
pub struct InternalRoutingRequest {
    pub payload: InternalRoutingRequestPayload,
    pub response_tx: Sender<InternalRoutingResponse>,
}

impl RequestToAdapter for InternalRoutingRequest {}

#[derive(Debug, PartialEq, Builder)]
pub struct InternalRoutingRequestPayload {
    pub person_id: String,
    pub from_link: String,
    pub to_link: String,
    pub mode: String,
    pub departure_time: u32,
    pub now: u32,
    #[builder(default = "Uuid::now_v7()")]
    pub uuid: Uuid,
}

impl InternalRoutingRequestPayload {
    pub fn equals_ignoring_uuid(&self, other: &Self) -> bool {
        self.person_id == other.person_id
            && self.from_link == other.from_link
            && self.to_link == other.to_link
            && self.mode == other.mode
            && self.departure_time == other.departure_time
            && self.now == other.now
    }
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
            now: req.now,
            request_id: req.uuid.as_bytes().to_vec(),
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
    config: Arc<Config>,
    shutdown_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl RoutingServiceAdapterFactory {
    pub fn new(
        ip: Vec<impl Into<String>>,
        config: Arc<Config>,
        shutdown_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    ) -> Self {
        Self {
            ip: ip.into_iter().map(|s| s.into()).collect(),
            config,
            shutdown_handles,
        }
    }
}

impl RequestAdapterFactory<InternalRoutingRequest> for RoutingServiceAdapterFactory {
    async fn build(self) -> impl RequestAdapter<InternalRoutingRequest> {
        let mut res = Vec::new();
        for ip in self.ip {
            info!("Connecting to routing service at {}", ip);
            let start = std::time::Instant::now();
            let client;
            loop {
                match RoutingServiceClient::connect(ip.clone()).await {
                    Ok(c) => {
                        client = c;
                        break;
                    }
                    Err(e) => {
                        if start.elapsed().as_secs()
                            >= self.config.computational_setup().retry_time_seconds
                        {
                            panic!(
                                "Failed to connect to routing service at {} after configured retry maximum: {}",
                                ip, e
                            );
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
            res.push(client);
        }
        RoutingServiceAdapter::new(res, self.shutdown_handles)
    }
}

impl RequestAdapter<InternalRoutingRequest> for RoutingServiceAdapter {
    fn on_request(&mut self, internal_req: InternalRoutingRequest) {
        let mut client = self.clients.next_cloned();

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

    fn on_shutdown(&mut self) {
        for client in &mut self.clients {
            let mut c = client.clone();
            let handle = tokio::spawn(async move {
                c.shutdown(())
                    .await
                    .expect("Error while shutting down routing service");
            });
            self.shutdown_handles.lock().unwrap().push(handle);
        }
    }
}

impl RoutingServiceAdapter {
    fn new(
        clients: Vec<RoutingServiceClient<tonic::transport::Channel>>,
        shutdown_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    ) -> Self {
        Self {
            clients: RingIter::new(clients),
            shutdown_handles,
        }
    }
}
