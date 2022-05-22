mod id;
mod context;

use std::net::UdpSocket;

use concurrency::{
  Service,
  StopRx,
};

use oddity_rtsp_protocol::ResponseMaybeInterleaved;

use oddity_video::{
  RtpMuxer,
  RtpBuf,
};

use super::media::{
  Source,
  SourceRx,
  SourceMsg,
};

use context::{
  Context,
  Destination,
  UdpDestination,
  TcpInterleavedDestination,
};

pub use id::SessionId;

pub struct Session {
  service: Option<Service>,
  source_rx: SourceRx,
}

impl Session {

  pub fn new(
    source: &mut Source,
    context: Context,
  ) -> Self {
    let service = Service::spawn({
      let source_rx = source.subscribe();
      move |stop| {
        match context.dest {
          Destination::Udp(dest) => {
            Self::run_udp(
              source_rx,
              context.muxer,
              dest,
              stop,
            )
          },
          Destination::TcpInterleaved(dest) => {
            Self::run_tcp_interleaved(
              source_rx,
              context.muxer,
              dest,
              stop,
            )
          }
        }
      }
    });

    Self {
      service: Some(service),
      source_rx: source.subscribe(),
    }
  }

  pub fn play() {
    // TODO
  }

  // TODO drop() = teardown (?)
  pub fn teardown(self) {
    
  }

  fn run_udp(
    source_rx: SourceRx,
    muxer: RtpMuxer,
    dest: UdpDestination,
    stop: StopRx,
  ) {
    // TODO
    // !!! How to handle `server_port` pair, do we need to receive data there and if so, what
    //  do we do with it ??!?!?!
    // okay i think we can get 'm like this: https://www.ffmpeg.org/doxygen/3.4/rtpproto_8h.html

    let socket_rtp = match UdpSocket::bind("0.0.0.0:0") {
      Ok(socket) => socket,
      Err(err) => {
        // TODO error
        return;
      },
    };

    let socket_rtcp = match UdpSocket::bind("0.0.0.0:0") {
      Ok(socket) => socket,
      Err(err) => {
        // TODO error
        return;
      },
    };

    loop {
      let msg = source_rx.recv();
      if let Ok(msg) = msg {
        match msg {
          SourceMsg::Init(stream_info) => {
            // TODO
          },
          SourceMsg::Packet(packet) => {
            match muxer.mux(packet) {
              Ok(output) => {
                match output {
                  RtpBuf::Rtp(buf) => {
                    socket_rtp.send_to(&buf, dest.rtp_remote).unwrap(); // TODO
                  },
                  RtpBuf::Rtcp(buf) => {
                    socket_rtp.send_to(&buf, dest.rtcp_remote).unwrap(); // TODO
                  }
                }
              },
              Err(err) => {
                // TODO
              },
            }
            // TODO
          },
        };
      } else {
        // TODO
      }
      /*
      channel::select! {
        recv(source_rx) -> msg => {
        },
        recv(stop.into_rx()) -> _ => {
          // TODO
          break;
        },
      };
      */
    }
  }

  fn run_tcp_interleaved(
    source_rx: SourceRx,
    muxer: RtpMuxer,
    dest: TcpInterleavedDestination,
    stop: StopRx,
  ) {
    loop {
      let msg = source_rx.recv();
      if let Ok(msg) = msg {
        match msg {
          SourceMsg::Init(stream_info) => {
            // TODO
          },
          SourceMsg::Packet(packet) => {
            match muxer.mux(packet) {
              Ok(output) => {
                let response_interleaved_message = match output {
                  RtpBuf::Rtp(buf) => {
                    ResponseMaybeInterleaved::Interleaved {
                      channel: dest.rtp_channel,
                      payload: buf.into(),
                    }
                  },
                  RtpBuf::Rtcp(buf) => {
                    ResponseMaybeInterleaved::Interleaved {
                      channel: dest.rtcp_channel,
                      payload: buf.into(),
                    }
                  },
                };
                dest.tx.send(response_interleaved_message).unwrap(); // TODO error handling
              },
              Err(err) => {
                // TODO
              },
            };
          },
        };
      } else {
        // TODO
      }
      /*
      channel::select! {
        recv(source_rx) -> msg => {
        },
        recv(stop.into_rx()) -> _ => {
          // TODO
          break;
        },
      };
      */
    }

  }

}