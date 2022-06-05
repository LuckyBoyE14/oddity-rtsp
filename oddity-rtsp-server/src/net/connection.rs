use std::fmt;
use std::sync::Arc;

use futures::SinkExt;

use tokio::select;
use tokio::sync::mpsc;
use tokio::net;
use tokio_stream::StreamExt;
use tokio_util::codec;

use rand::random;

use oddity_rtsp_protocol::{Codec, AsServer, ResponseMaybeInterleaved};

use crate::runtime::Runtime;
use crate::runtime::task_manager::{Task, TaskContext};
use crate::net::handler::Handler;

pub enum ConnectionState {
  Disconnected(ConnectionId),
  Closed(ConnectionId),
}

pub type ConnectionStateTx = mpsc::UnboundedSender<ConnectionState>;
pub type ConnectionStateRx = mpsc::UnboundedReceiver<ConnectionState>;

pub type ResponseSenderTx = mpsc::UnboundedSender<ResponseMaybeInterleaved>;
pub type ResponseSenderRx = mpsc::UnboundedReceiver<ResponseMaybeInterleaved>;

pub struct Connection {
  sender_tx: ResponseSenderTx,
  worker: Task,
}

impl Connection {

  pub async fn start(
    id: ConnectionId,
    inner: net::TcpStream,
    handler: Arc<Handler>,
    state_tx: ConnectionStateTx,
    runtime: &Runtime,
  ) -> Self {
    let (sender_tx, sender_rx) = mpsc::unbounded_channel();

    tracing::trace!(%id, "starting connection");
    let worker = runtime
      .task()
      .spawn({
        let sender_tx = sender_tx.clone();
        move |task_context| Self::run(
          id,
          inner,
          handler,
          state_tx,
          sender_tx,
          sender_rx,
          task_context,
        )
      })
      .await;
    tracing::trace!(%id, "started connection");

    Connection {
      sender_tx,
      worker,
    }
  }
  
  pub async fn close(&mut self) {
    tracing::trace!("closing connection");
    self.worker.stop().await;
    tracing::trace!("closed connection");
  }

  pub fn sender_tx(&self) -> ResponseSenderTx {
    self.sender_tx.clone()
  }

  async fn run(
    id: ConnectionId,
    inner: net::TcpStream,
    handler: Arc<Handler>,
    state_tx: ConnectionStateTx,
    response_tx: ResponseSenderTx,
    mut response_rx: ResponseSenderRx,
    mut task_context: TaskContext,
  ) {
    let mut disconnected = false;

    let addr = inner
      .peer_addr()
      .map(|peer_addr| peer_addr.to_string())
      .unwrap_or("?".to_string());
    let (read, write) = inner.into_split();
    let mut inbound = codec::FramedRead::new(read, Codec::<AsServer>::new());
    let mut outbound = codec::FramedWrite::new(write, Codec::<AsServer>::new());

    loop {
      select! {
        message = response_rx.recv() => {
          match message {
            Some(message) => {
              if let Err(err) = outbound.send(message).await {
                tracing::error!(%err, %id, %addr, "connection: failed to send message");
                break;
              }
            },
            None => {
              break;
            },
          }
        },
        request = inbound.next() => {
          match request {
            Some(Ok(request)) => {
              // TODO can we make it so that it only is cloned when really needed
              // since this is just wasteful
              let response = handler.handle(&request, response_tx.clone()).await;
              let response = ResponseMaybeInterleaved::Message(response);
              if let Err(err) = outbound.send(response).await {
                tracing::error!(%err, %id, %addr, "connection: failed to send response");
                break;
              }
            },
            Some(Err(err)) => {
              tracing::error!(%err, %id, %addr, "connection: failed to read request");
              break;
            },
            None => {
              disconnected = true;
              tracing::info!(%id, %addr, "connection: client disconnected");
              break;
            },
          }
        },
        _ = task_context.wait_for_stop() => {
          tracing::trace!(%id, %addr, "connection worker stopping");
          break;
        },
      };
    }

    if disconnected {
      // Client disconnected.
      let _ = state_tx.send(ConnectionState::Disconnected(id));
    } else {
      // Reason for breaking out of loop was unexpected and not due to the
      // client disconnecting.
      let _ = state_tx.send(ConnectionState::Closed(id));
    }
    tracing::trace!(%id, %addr, "connection worker EOL");
  }

}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ConnectionId(usize);

impl ConnectionId {

  pub fn generate() -> Self {
    Self(random())
  }

}

impl fmt::Display for ConnectionId {

  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.0)
  }

}

pub struct ConnectionIdGenerator(usize);

impl ConnectionIdGenerator {

  pub fn new() -> Self {
    ConnectionIdGenerator(0)
  }

  pub fn generate(&mut self) -> ConnectionId {
    let id = self.0;
    self.0 += 1;
    ConnectionId(id)
  }

}