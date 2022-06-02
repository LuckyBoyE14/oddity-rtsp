use std::fmt;
use std::net::SocketAddr;

use oddity_video as video;
use oddity_rtsp_protocol as rtsp;

use crate::net::connection::ResponseSenderTx;
use crate::session::transport;

pub struct SessionSetup {
  rtsp_transport: rtsp::Transport,
  rtp_muxer: video::RtpMuxer,
  rtp_target: SessionSetupTarget,
}

impl SessionSetup {

  pub fn from_rtsp_candidate_transports(
    candidate_transports: impl IntoIterator<Item=rtsp::Transport>,
    sender: ResponseSenderTx,
  ) -> Result<Self, SessionSetupError> {
    let transport = candidate_transports
      .into_iter()
      .filter(|transport| transport::is_supported(&transport))
      .next()
      .ok_or_else(|| SessionSetupError::TransportNotSupported)?;
    
    video::RtpMuxer::new()
      .map_err(SessionSetupError::Media)
      .and_then(|rtp_muxer| {
        let rtsp_transport = transport::resolve_transport(&transport, &rtp_muxer);
        let rtp_target = SessionSetupTarget::from_rtsp_transport(&transport, sender)
          .ok_or_else(|| SessionSetupError::DestinationInvalid)?;

        Ok(Self {
          rtsp_transport,
          rtp_muxer,
          rtp_target,
        })
      })
  }

}

pub enum SessionSetupTarget {
  RtpUdp(SendOverSocket),
  RtpTcp(SendInterleaved),
}

pub struct SendOverSocket {
  pub rtp_remote: SocketAddr,
  pub rtcp_remote: SocketAddr,
}

pub struct SendInterleaved {
  pub sender: ResponseSenderTx,
  pub rtp_channel: u8,
  pub rtcp_channel: u8,
}

impl SessionSetupTarget {

  pub fn from_rtsp_transport(
    rtsp_transport: &rtsp::Transport,
    sender: ResponseSenderTx,
  ) -> Option<Self> {
    Some(
      match rtsp_transport.lower_protocol()? {
        rtsp::Lower::Udp => {
          let client_ip_addr = rtsp_transport.destination()?;
          let (client_rtp_port, client_rtcp_port) =
            match rtsp_transport.client_port()? {
              rtsp::Port::Single(rtp_port)
                => (*rtp_port, rtp_port + 1),
              rtsp::Port::Range(rtp_port, rtcp_port)
                => (*rtp_port, *rtcp_port),
            };

          SessionSetupTarget::RtpUdp(
            SendOverSocket {
              rtp_remote: (*client_ip_addr, client_rtp_port).into(),
              rtcp_remote: (*client_ip_addr, client_rtcp_port).into(),
            }
          )
        },
        rtsp::Lower::Tcp => {
          let (rtp_channel, rtcp_channel) =
            match rtsp_transport.interleaved_channel()? {
              rtsp::Channel::Single(rtp_channel)
                => (*rtp_channel, rtp_channel + 1),
              rtsp::Channel::Range(rtp_channel, rtcp_channel)
                => (*rtp_channel, *rtcp_channel),
            };

          SessionSetupTarget::RtpTcp(
            SendInterleaved {
              sender,
              rtp_channel,
              rtcp_channel,
            }
          )
        },
      }
    )
  }
}

pub enum SessionSetupError {
  TransportNotSupported,
  DestinationInvalid,
  Media(video::Error),
}

impl fmt::Display for SessionSetupError {

  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      SessionSetupError::TransportNotSupported => write!(f, "transport not supported"),
      SessionSetupError::DestinationInvalid  => write!(f, "destination invalid"),
      SessionSetupError::Media(error) => write!(f, "media error: {}", error),
    }
  }

}