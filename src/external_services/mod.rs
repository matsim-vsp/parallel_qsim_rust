use tokio::sync::mpsc::Receiver;
use tracing::info;

pub mod routing;

trait RequestHandler<T> {
    async fn on_request(&mut self, req: T);
}

fn execute_adapter<T>(
    mut receiver: Receiver<T>,
    mut req_handler: impl RequestHandler<T>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    info!("Starting adapter");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        println!("Shutdown signal received, exiting adapter.");
                        break;
                    }
                }
                maybe_req = receiver.recv() => {
                    if let Some(req) = maybe_req {
                        req_handler.on_request(req).await;
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
        let handler = MockRequestHandler(counter.clone());
        let (shoutdown_send, shutdown_recv) = tokio::sync::watch::channel(false);

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
        shoutdown_send.send(true).unwrap();

        handle.join().unwrap();
    }

    struct MockRequest {
        payload: String,
        response_tx: tokio::sync::oneshot::Sender<String>,
    }

    struct MockRequestHandler(Arc<Mutex<usize>>);

    impl RequestHandler<MockRequest> for MockRequestHandler {
        async fn on_request(&mut self, req: MockRequest) {
            println!("Mock handler received request: {}", req.payload);
            let mut guard = self.0.lock().unwrap();
            *guard += 1;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; // Simulate some processing delay
            req.response_tx.send(String::from("Ok")).unwrap();
        }
    }
}
