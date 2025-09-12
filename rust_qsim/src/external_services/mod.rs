use derive_builder::Builder;
use std::fmt::Debug;
use std::thread::JoinHandle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::info;

pub mod routing;

/// This trait is a marker trait for requests that can be sent to an adapter.
pub trait RequestToAdapter: Debug + Send {}

/// This struct is a wrapper around the JoinHandle of the adapter thread. Additionally, it holds a shutdown sender for the adapter.
/// The purpose of this struct is to manage the lifecycle of the adapter thread, allowing for sending shutdown signals before waiting for the thread to finish.
#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct AdapterHandle {
    pub(super) handle: JoinHandle<()>,
    pub(super) shutdown_sender: tokio::sync::watch::Sender<bool>,
}

/// This enum defines the types of external services that can be used in the simulation.
/// It works as a key for different service adapters in the simulation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExternalServiceType {
    Routing(String),
}

/// This trait defines a factory for creating request adapters.
pub trait RequestAdapterFactory<T: RequestToAdapter> {
    /// This method builds the request adapter. It returns a future that resolves to the adapter instance.
    fn build(self) -> impl std::future::Future<Output = impl RequestAdapter<T>>;

    /// This method creates a channel for sending requests to the adapter.
    fn request_channel(&self, buffer: usize) -> (Sender<T>, Receiver<T>) {
        mpsc::channel(buffer)
    }

    /// This method creates a shutdown channel for the adapter.
    fn shutdown_channel(
        &self,
    ) -> (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ) {
        tokio::sync::watch::channel(false)
    }

    /// This method returns the number of *additional* threads to be used for the tokio runtime by the adapter.
    fn thread_count(&self) -> usize {
        1
    }
}

/// This trait defines the behavior of a request adapter. A request adapter processes incoming requests of type T.
/// One request adapter instance is run in a separate thread with its own tokio runtime. It might use multiple threads internally for the tokio runtime.
pub trait RequestAdapter<T: RequestToAdapter> {
    fn on_request(&mut self, req: T) -> impl std::future::Future<Output = ()>;
    fn on_shutdown(&mut self) {
        info!("Adapter is shutting down");
    }
}

/// This function executes the adapter in a separate thread with its own tokio runtime.
pub fn execute_adapter<T: RequestToAdapter>(
    mut receiver: Receiver<T>,
    req_adapter_factory: impl RequestAdapterFactory<T>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    info!("Starting adapter");

    assert!(
        req_adapter_factory.thread_count() > 0,
        "routing.thread_count must be greater than 0"
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(req_adapter_factory.thread_count())
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let mut req_adapter = req_adapter_factory.build().await;

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Shutdown signal received, exiting adapter.");
                        req_adapter.on_shutdown();
                        break;
                    }
                }
                maybe_req = receiver.recv() => {
                    if let Some(req) = maybe_req {
                        req_adapter.on_request(req).await;
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use tokio::sync::mpsc;

    #[test]
    fn test_execute_adapter() {
        let (tx, rx) = mpsc::channel(10);
        let counter = Arc::new(Mutex::new(0));
        let handler = MockRequestAdapterBuilder(counter.clone());
        let (shutdown_send, shutdown_recv) = tokio::sync::watch::channel(false);

        // Spawn the adapter in a separate task
        let handle = thread::spawn(move || {
            execute_adapter(rx, handler, shutdown_recv);
        });

        // Send a request
        let (send, recv) = tokio::sync::oneshot::channel();
        tx.blocking_send(MockRequest {
            payload: String::from("Test Payload"),
            response_tx: send,
        })
        .unwrap();

        let string = recv.blocking_recv().unwrap();
        assert_eq!(string, String::from("Ok"));
        assert_eq!(*counter.lock().unwrap(), 1);
        shutdown_send.send(true).unwrap();

        handle.join().unwrap();
    }

    #[derive(Debug)]
    struct MockRequest {
        payload: String,
        response_tx: tokio::sync::oneshot::Sender<String>,
    }

    impl RequestToAdapter for MockRequest {}

    struct MockRequestAdapter(Arc<Mutex<usize>>);

    impl RequestAdapter<MockRequest> for MockRequestAdapter {
        async fn on_request(&mut self, req: MockRequest) {
            println!("Mock handler received request: {}", req.payload);
            {
                let mut guard = self.0.lock().unwrap();
                *guard += 1;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; // Simulate some processing delay
            req.response_tx.send(String::from("Ok")).unwrap();
        }
    }

    struct MockRequestAdapterBuilder(Arc<Mutex<usize>>);

    impl RequestAdapterFactory<MockRequest> for MockRequestAdapterBuilder {
        async fn build(self) -> impl RequestAdapter<MockRequest> {
            MockRequestAdapter(self.0)
        }
    }
}
