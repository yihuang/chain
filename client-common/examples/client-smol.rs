use std::cell::RefCell;
use std::collections::HashMap;
use std::net::TcpStream;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use easy_parallel::Parallel;
use futures::channel::{mpsc, oneshot};
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use smol::Async;
use tungstenite::Message;

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub id: String,
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    pub id: String,
    pub jsonrpc: String,
    pub error: Option<JsonRpcError>,
    pub result: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

type InnerRequest = (JsonRpcRequest, oneshot::Sender<JsonRpcResponse>);

async fn client_loop(mut req_chan: mpsc::Receiver<InnerRequest>) -> Result<()> {
    let stream = Async::<TcpStream>::connect("127.0.0.1:26657").await?;
    let (stream, _resp) =
        async_tungstenite::client_async("ws://127.0.0.1:26657/websocket", stream).await?;
    let (mut writer, mut reader) = stream.split();

    let listeners = Rc::new(RefCell::new(<HashMap<
        String,
        oneshot::Sender<JsonRpcResponse>,
    >>::new()));

    let listeners_ = listeners.clone();
    let _recv_task = smol::Task::local(async move {
        loop {
            let rsp = reader.next().await.unwrap();
            if let Message::Text(txt) = rsp.expect("tungstenite error") {
                if let Ok(rsp) = serde_json::from_str::<JsonRpcResponse>(&txt) {
                    if let Some(listener) = listeners_.borrow_mut().remove(&rsp.id) {
                        let _ = listener.send(rsp);
                    } else {
                        println!("unknown response.id: {}", rsp.id);
                    }
                } else {
                    println!("unknown text message response: {}", txt);
                }
            } else {
                println!("websocket response is not text message");
            }
        }
    });

    while let Some((req, listener)) = req_chan.next().await {
        listeners.borrow_mut().insert(req.id.clone(), listener);
        writer
            .send(Message::text(serde_json::to_string(&req).unwrap()))
            .await?
    }
    writer.close().await?;

    println!("websocket thread quit");
    Ok(())
}

#[derive(Debug, Clone)]
struct WSClient {
    id_gen: Arc<AtomicUsize>,
    sender: mpsc::Sender<InnerRequest>,
}

impl WSClient {
    pub fn new(sender: mpsc::Sender<InnerRequest>) -> Self {
        WSClient {
            id_gen: Arc::new(AtomicUsize::new(0)),
            sender,
        }
    }

    pub fn call(&mut self, method: String, params: Vec<Value>) -> JsonRpcResponse {
        let id = self.id_gen.fetch_add(1, Ordering::Relaxed).to_string();
        let req = JsonRpcRequest {
            id,
            jsonrpc: "2.0".to_owned(),
            method,
            params,
        };
        let (sender, receiver) = oneshot::channel::<JsonRpcResponse>();
        smol::block_on(async {
            self.sender
                .send((req, sender))
                .await
                .expect("request channel is closed unexpectedly");
            receiver
                .await
                .expect("oneshot channel is closed unexpectedly")
        })
    }
}

fn main() {
    let (sender, receiver) = mpsc::channel(100);
    let ws_thread = std::thread::spawn(move || {
        smol::run(client_loop(receiver)).expect("websocket thread quit unexpectedly");
    });
    let mut client = WSClient::new(sender);
    Parallel::new()
        .each(0..10, {
            let mut client_ = client.clone();
            move |_| {
                dbg!(client_.call("status".to_owned(), vec![]));
            }
        })
        .run();

    client.sender.close_channel();
    ws_thread.join().unwrap()
}
