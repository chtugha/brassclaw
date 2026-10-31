//! Real HTTP/2 REFUSED_STREAM after reading the complete submitted body.
//! The default-policy control proves the fixture actually triggers a resend.
use super::*;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

const TEST_DEADLINE: Duration = Duration::from_secs(5);
struct Peer {
    address: SocketAddr,
    bodies: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: oneshot::Sender<()>,
    driver: JoinHandle<()>,
}
async fn peer() -> Peer {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind HTTP/2 fixture");
    let address = listener.local_addr().expect("fixture address");
    let bodies = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let received = bodies.clone();
    let (stop, mut stopped) = oneshot::channel();
    let driver = tokio::spawn(async move {
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                socket=listener.accept()=>{
                    let (socket,_)=socket.expect("accept HTTP/2 fixture socket");
                    let received=received.clone();
                    connections.spawn(async move {
                        let mut connection=h2::server::handshake(socket).await.expect("HTTP/2 handshake");
                        let mut handlers=JoinSet::new();
                        loop {
                            tokio::select! {
                                request=connection.accept()=>{
                                    let Some(request)=request else {break;};
                                    let (mut request,mut respond)=request.expect("receive HTTP/2 stream");
                                    let received=received.clone();
                                    handlers.spawn(async move {
                                        let mut bytes=Vec::new();
                                        while let Some(chunk)=request.body_mut().data().await {
                                            let chunk=chunk.expect("read complete submitted body");
                                            request.body_mut().flow_control().release_capacity(chunk.len()).expect("return flow capacity");
                                            bytes.extend_from_slice(&chunk);
                                        }
                                        let first={
                                            let mut bodies=received.lock().expect("captured body lock");
                                            bodies.push(bytes);bodies.len()==1
                                        };
                                        if first {respond.send_reset(h2::Reason::REFUSED_STREAM);}
                                        else {respond.send_response(http::Response::new(()),true).expect("complete control response");}
                                    });
                                }
                                completed=handlers.join_next(),if !handlers.is_empty()=>{
                                    completed.expect("registered body task").expect("body task succeeds");
                                }
                            }
                        }
                        while let Some(completed)=handlers.join_next().await {completed.expect("body task succeeds");}
                    });
                }
                completed=connections.join_next(),if !connections.is_empty()=>{
                    completed.expect("registered connection task").expect("connection task succeeds");
                }
                // Dropping the owner is also an explicit fixture shutdown.
                _=&mut stopped=>break,
            }
        }
        connections.abort_all();
        while let Some(completed) = connections.join_next().await {
            if let Err(error) = completed {
                assert!(
                    error.is_cancelled(),
                    "unexpected native fixture failure: {error}"
                );
            }
        }
    });
    Peer {
        address,
        bodies,
        stop,
        driver,
    }
}

#[tokio::test]
async fn actual_transport_does_not_resubmit_a_body_after_http2_protocol_nack() {
    for hardened in [false, true] {
        tokio::time::timeout(TEST_DEADLINE, async {
            let Peer {
                address,
                bodies,
                stop,
                driver,
            } = peer().await;
            let key = ReqwestClientKey {
                host: "retry.fixture.test".into(),
                port: address.port(),
                resolved_addrs: vec![address],
                timeout: TEST_DEADLINE,
            };
            let builder = if hardened {
                reqwest_client_builder(&key)
            } else {
                // Positive control keeps Reqwest's actual default protocol policy.
                reqwest::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(key.timeout)
                    .resolve_to_addrs(&key.host, &key.resolved_addrs)
            };
            // Select HTTP/2 for the local plaintext fixture, without weakening
            // TLS validation or changing production protocol negotiation.
            let client = builder
                .http2_prior_knowledge()
                .build()
                .expect("native pinned client");
            let transport = ReqwestNetworkTransport::new(TEST_DEADLINE);
            {
                let mut cache = transport.client_cache.lock().expect("native cache");
                cache.entries.put(key, client);
            }
            let body = b"refresh_token=controlled-issuer-token&grant_type=refresh_token".to_vec();
            let result = transport
                .execute(NetworkTransportRequest {
                    method: NetworkMethod::Post,
                    url: format!("http://retry.fixture.test:{}/token", address.port()),
                    headers: vec![(
                        "content-type".into(),
                        "application/x-www-form-urlencoded".into(),
                    )],
                    body: body.clone(),
                    resolved_ips: vec![address.ip()],
                    response_body_limit: Some(1024),
                    timeout_ms: Some(
                        TEST_DEADLINE
                            .as_millis()
                            .try_into()
                            .expect("fixture duration fits the request type"),
                    ),
                })
                .await;
            // The client result implies that the reset/success has been driven
            // onto the wire, so shutdown needs no timing sleeps or polling.
            stop.send(()).expect("fixture remains supervised");
            driver.await.expect("fixture joins all connection tasks");
            let captured = bodies.lock().expect("captured request bodies").clone();
            assert!(captured.iter().all(|value| value == &body));
            if hardened {
                assert_eq!(
                    captured.len(),
                    1,
                    "one actual submission belongs to one dispatch"
                );
                let NetworkHttpError::Transport {
                    request_bytes,
                    response_bytes,
                    ..
                } = result.expect_err("NACK remains an error")
                else {
                    panic!("protocol NACK must retain the classified transport error");
                };
                assert_eq!(request_bytes, body.len() as u64);
                assert_eq!(response_bytes, 0);
            } else {
                assert_eq!(
                    result.expect("default policy repeats the request").status,
                    200
                );
                assert_eq!(
                    captured.len(),
                    2,
                    "control must actually observe the hidden default resend"
                );
            }
        })
        .await
        .expect("native HTTP/2 fixture completes within its test bound");
    }
}
