use crate::external_services::{RequestAdapter, RequestToAdapter};
use crate::generated::routing::routing_service_client::RoutingServiceClient;
use crate::generated::routing::{Request, Response};
use crate::simulation::population::{InternalActivity, InternalLeg, InternalPlanElement};
use itertools::{EitherOrBoth, Itertools};
use tokio::runtime::Runtime;
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

#[derive(Debug, Clone)]
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

impl RoutingServiceAdapter {
    pub fn new(ip: &str) -> Self {
        let client = Runtime::new().unwrap().block_on(async {
            RoutingServiceClient::connect(ip.to_string())
                .await
                .expect("Failed to connect to routing service")
        });
        Self { client }
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
