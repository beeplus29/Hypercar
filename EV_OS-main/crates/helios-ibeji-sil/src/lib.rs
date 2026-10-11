//! A local Ibeji-compatible SIL service pair. It is not a replacement for the
//! upstream in-vehicle digital-twin service or a production identity registry.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status, transport::Server};

pub mod invehicle_digital_twin {
    tonic::include_proto!("invehicle_digital_twin");
}

pub mod digital_twin_provider {
    tonic::include_proto!("digital_twin_provider");
}

pub mod digital_twin_consumer {
    tonic::include_proto!("digital_twin_consumer");
}

use digital_twin_consumer::digital_twin_consumer_client::DigitalTwinConsumerClient;
use digital_twin_provider::digital_twin_provider_server::{DigitalTwinProvider, DigitalTwinProviderServer};
use digital_twin_provider::{
    GetRequest, GetResponse, InvokeRequest, InvokeResponse, SetRequest, SetResponse, StreamRequest,
    StreamResponse, SubscribeRequest, SubscribeResponse, UnsubscribeRequest, UnsubscribeResponse,
};
use invehicle_digital_twin::invehicle_digital_twin_client::InvehicleDigitalTwinClient;
use invehicle_digital_twin::invehicle_digital_twin_server::{
    InvehicleDigitalTwin, InvehicleDigitalTwinServer,
};
use invehicle_digital_twin::{
    EndpointInfo, EntityAccessInfo, FindByIdRequest, FindByIdResponse, RegisterRequest, RegisterResponse,
};

pub const TELEMETRY_ENTITY_ID: &str = "urn:helios:vehicle:1:telemetry";
pub const REGISTRY_OPERATION: &str = "Register";
pub const REGISTRY_PROTOCOL: &str = "grpc";
const MAX_CONSUMERS_PER_ENTITY: usize = 16;
const MAX_JSON_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Default)]
struct Registry {
    entities: Arc<RwLock<HashMap<String, EntityAccessInfo>>>,
}

#[tonic::async_trait]
impl InvehicleDigitalTwin for Registry {
    async fn find_by_id(
        &self,
        request: Request<FindByIdRequest>,
    ) -> Result<Response<FindByIdResponse>, Status> {
        let id = request.into_inner().id;
        let info = self.entities.read().await.get(&id).cloned();
        Ok(Response::new(FindByIdResponse { entity_access_info: info }))
    }

    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let infos = request.into_inner().entity_access_info_list;
        if infos.is_empty() {
            return Err(Status::invalid_argument("registration must include an entity"));
        }
        if infos.iter().any(|info| {
            info.id.is_empty()
                || info.name.is_empty()
                || info.endpoint_info_list.is_empty()
                || info.endpoint_info_list.iter().any(|endpoint| {
                    endpoint.protocol != REGISTRY_PROTOCOL
                        || endpoint.uri.is_empty()
                        || !endpoint.operations.iter().any(|op| op == "Subscribe")
                })
        }) {
            return Err(Status::invalid_argument(
                "entity registration requires an id, name, and grpc Subscribe endpoint",
            ));
        }
        let mut entities = self.entities.write().await;
        for info in infos {
            entities.insert(info.id.clone(), info);
        }
        Ok(Response::new(RegisterResponse {}))
    }
}

#[derive(Clone, Debug, Default)]
struct Provider {
    subscribers: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

#[tonic::async_trait]
impl DigitalTwinProvider for Provider {
    type StreamStream = tokio_stream::wrappers::ReceiverStream<Result<StreamResponse, Status>>;

    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<SubscribeResponse>, Status> {
        let request = request.into_inner();
        if request.entity_id != TELEMETRY_ENTITY_ID {
            return Err(Status::not_found("unknown telemetry entity id"));
        }
        let uri = tonic::transport::Endpoint::from_shared(request.consumer_uri.clone())
            .map_err(|_| Status::invalid_argument("consumer_uri must be a valid HTTP(S) URI"))?;
        drop(uri);
        if !(request.consumer_uri.starts_with("http://127.0.0.1:")
            || request.consumer_uri.starts_with("http://localhost:")
            || request.consumer_uri.starts_with("http://[::1]:"))
        {
            return Err(Status::invalid_argument(
                "the hardware-free SIL provider accepts loopback consumers only",
            ));
        }

        let mut subscribers = self.subscribers.write().await;
        let consumers = subscribers.entry(request.entity_id).or_default();
        if !consumers.contains(&request.consumer_uri) && consumers.len() >= MAX_CONSUMERS_PER_ENTITY {
            return Err(Status::resource_exhausted("SIL consumer limit reached for telemetry entity"));
        }
        consumers.insert(request.consumer_uri);
        Ok(Response::new(SubscribeResponse {}))
    }

    async fn unsubscribe(
        &self,
        request: Request<UnsubscribeRequest>,
    ) -> Result<Response<UnsubscribeResponse>, Status> {
        let request = request.into_inner();
        let mut subscribers = self.subscribers.write().await;
        if let Some(consumers) = subscribers.get_mut(&request.entity_id) {
            consumers.remove(&request.consumer_uri);
        }
        Ok(Response::new(UnsubscribeResponse {}))
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let entity_id = request.into_inner().entity_id;
        if entity_id != TELEMETRY_ENTITY_ID {
            return Err(Status::not_found("unknown telemetry entity id"));
        }
        Err(Status::unimplemented(
            "the Ibeji provider wire contract returns no value in GetResponse; subscribe to telemetry instead",
        ))
    }

    async fn set(&self, _request: Request<SetRequest>) -> Result<Response<SetResponse>, Status> {
        Err(Status::permission_denied(
            "simulation telemetry is read-only; no actuator or vehicle commands are accepted",
        ))
    }

    async fn invoke(&self, _request: Request<InvokeRequest>) -> Result<Response<InvokeResponse>, Status> {
        Err(Status::permission_denied("the SIL telemetry provider exposes no vehicle operations"))
    }

    async fn stream(&self, request: Request<StreamRequest>) -> Result<Response<Self::StreamStream>, Status> {
        let entity_id = request.into_inner().entity_id;
        if entity_id != TELEMETRY_ENTITY_ID {
            return Err(Status::not_found("unknown telemetry entity id"));
        }
        Err(Status::unimplemented(
            "this SIL provider publishes updates through the Ibeji Subscribe/Publish flow",
        ))
    }
}

#[derive(Debug)]
pub struct IbejiSil {
    registry_uri: String,
    provider_uri: String,
    registry_task: JoinHandle<()>,
    provider_task: JoinHandle<()>,
    provider: Provider,
}

impl IbejiSil {
    pub async fn start() -> anyhow::Result<Self> {
        let registry_listener = TcpListener::bind("127.0.0.1:0").await?;
        let registry_addr = registry_listener.local_addr()?;
        let provider_listener = TcpListener::bind("127.0.0.1:0").await?;
        let provider_addr = provider_listener.local_addr()?;

        let registry_uri = format!("http://{registry_addr}");
        let provider_uri = format!("http://{provider_addr}");
        let registry = Registry::default();
        let provider = Provider::default();

        let registry_task = tokio::spawn({
            let registry = registry.clone();
            async move {
                if let Err(error) = Server::builder()
                    .add_service(InvehicleDigitalTwinServer::new(registry))
                    .serve_with_incoming(TcpListenerStream::new(registry_listener))
                    .await
                {
                    eprintln!("Ibeji SIL registry stopped: {error}");
                }
            }
        });
        let provider_task = tokio::spawn({
            let provider = provider.clone();
            async move {
                if let Err(error) = Server::builder()
                    .add_service(DigitalTwinProviderServer::new(provider))
                    .serve_with_incoming(TcpListenerStream::new(provider_listener))
                    .await
                {
                    eprintln!("Ibeji SIL provider stopped: {error}");
                }
            }
        });

        let mut client = InvehicleDigitalTwinClient::connect(registry_uri.clone()).await?;
        client
            .register(RegisterRequest {
                entity_access_info_list: vec![EntityAccessInfo {
                    name: "HELIOS vehicle telemetry".into(),
                    id: TELEMETRY_ENTITY_ID.into(),
                    description: "Read-only telemetry from the HELIOS digital-twin simulation".into(),
                    endpoint_info_list: vec![EndpointInfo {
                        protocol: REGISTRY_PROTOCOL.into(),
                        operations: vec!["Subscribe".into()],
                        uri: provider_uri.clone(),
                        context: TELEMETRY_ENTITY_ID.into(),
                    }],
                }],
            })
            .await?;

        Ok(Self { registry_uri, provider_uri, registry_task, provider_task, provider })
    }

    pub fn registry_uri(&self) -> &str {
        &self.registry_uri
    }

    pub fn provider_uri(&self) -> &str {
        &self.provider_uri
    }

    pub async fn publish<T: Serialize>(&self, snapshot: &T) -> anyhow::Result<usize> {
        let payload = serde_json::to_string(snapshot)?;
        anyhow::ensure!(payload.len() <= MAX_JSON_BYTES, "telemetry exceeds SIL payload limit");
        let consumers: Vec<String> = self
            .provider
            .subscribers
            .read()
            .await
            .get(TELEMETRY_ENTITY_ID)
            .into_iter()
            .flatten()
            .cloned()
            .collect();

        let mut published = 0;
        for uri in consumers {
            let mut client = DigitalTwinConsumerClient::connect(uri.clone())
                .await
                .map_err(|error| anyhow::anyhow!("connect Ibeji SIL consumer {uri}: {error}"))?;
            client
                .publish(digital_twin_consumer::PublishRequest {
                    entity_id: TELEMETRY_ENTITY_ID.into(),
                    value: payload.clone(),
                })
                .await
                .map_err(|error| anyhow::anyhow!("publish Ibeji SIL telemetry to {uri}: {error}"))?;
            published += 1;
        }
        Ok(published)
    }
}

impl Drop for IbejiSil {
    fn drop(&mut self) {
        self.registry_task.abort();
        self.provider_task.abort();
    }
}

pub async fn find_telemetry_entity(registry_uri: &str) -> anyhow::Result<Option<EntityAccessInfo>> {
    let mut client = InvehicleDigitalTwinClient::connect(registry_uri.to_owned()).await?;
    let response = client.find_by_id(FindByIdRequest { id: TELEMETRY_ENTITY_ID.into() }).await?.into_inner();
    Ok(response.entity_access_info)
}

pub async fn subscribe_to_telemetry(provider_uri: &str, consumer_uri: &str) -> anyhow::Result<()> {
    let mut client = digital_twin_provider::digital_twin_provider_client::DigitalTwinProviderClient::connect(
        provider_uri.to_owned(),
    )
    .await?;
    client
        .subscribe(SubscribeRequest {
            entity_id: TELEMETRY_ENTITY_ID.into(),
            consumer_uri: consumer_uri.into(),
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use digital_twin_consumer::digital_twin_consumer_server::{
        DigitalTwinConsumer, DigitalTwinConsumerServer,
    };
    use digital_twin_consumer::{PublishRequest, PublishResponse, RespondRequest, RespondResponse};
    use serde::{Deserialize, Serialize};
    use tokio::net::TcpListener;
    use tokio_stream::wrappers::TcpListenerStream;

    #[derive(Clone)]
    struct TestConsumer {
        messages: tokio::sync::mpsc::UnboundedSender<String>,
    }

    #[derive(Deserialize, Serialize)]
    struct TestSnapshot {
        schema_version: u16,
        timestamp_ns: u64,
        speed_mps: f32,
        motor_torque_nm: [f32; 4],
    }

    #[tonic::async_trait]
    impl DigitalTwinConsumer for TestConsumer {
        async fn publish(
            &self,
            request: Request<PublishRequest>,
        ) -> Result<Response<PublishResponse>, Status> {
            let request = request.into_inner();
            if request.entity_id != TELEMETRY_ENTITY_ID {
                return Err(Status::not_found("unexpected entity"));
            }
            self.messages.send(request.value).map_err(|_| Status::unavailable("test receiver closed"))?;
            Ok(Response::new(PublishResponse {}))
        }

        async fn respond(
            &self,
            _request: Request<RespondRequest>,
        ) -> Result<Response<RespondResponse>, Status> {
            Err(Status::unimplemented("test consumer has no invoke operations"))
        }
    }

    #[tokio::test]
    async fn registers_entity_and_delivers_ibeji_publish_to_subscriber() {
        let ibeji = IbejiSil::start().await.expect("start local Ibeji SIL");
        let entity = find_telemetry_entity(ibeji.registry_uri())
            .await
            .expect("find entity")
            .expect("registered telemetry entity");
        assert_eq!(entity.id, TELEMETRY_ENTITY_ID);
        assert_eq!(entity.endpoint_info_list[0].protocol, "grpc");

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind consumer");
        let consumer_uri = format!("http://{}", listener.local_addr().expect("consumer address"));
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let consumer_task = tokio::spawn(async move {
            Server::builder()
                .add_service(DigitalTwinConsumerServer::new(TestConsumer { messages: sender }))
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .expect("serve test consumer");
        });

        subscribe_to_telemetry(ibeji.provider_uri(), &consumer_uri).await.expect("subscribe consumer");
        let snapshot = TestSnapshot {
            schema_version: 1,
            timestamp_ns: 77,
            speed_mps: 42.5,
            motor_torque_nm: [1.0, 2.0, 3.0, 4.0],
        };
        assert_eq!(ibeji.publish(&snapshot).await.expect("publish"), 1);
        let json = receiver.recv().await.expect("received consumer update");
        let received: TestSnapshot = serde_json::from_str(&json).expect("parse telemetry JSON");
        assert_eq!(received.timestamp_ns, snapshot.timestamp_ns);
        assert_eq!(received.speed_mps, snapshot.speed_mps);
        assert_eq!(received.motor_torque_nm, snapshot.motor_torque_nm);

        let mut provider =
            digital_twin_provider::digital_twin_provider_client::DigitalTwinProviderClient::connect(
                ibeji.provider_uri().to_owned(),
            )
            .await
            .expect("connect provider");
        let set_result =
            provider.set(SetRequest { entity_id: TELEMETRY_ENTITY_ID.into(), value: "{}".into() }).await;
        assert_eq!(set_result.expect_err("SIL must reject Set").code(), tonic::Code::PermissionDenied);
        consumer_task.abort();
    }
}
