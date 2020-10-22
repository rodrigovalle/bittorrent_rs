use crate::Opt;

use std::net::IpAddr;
use std::sync::Arc;
use hyper::{Body, Method, Request};
use serde::{Serialize, Deserialize};
use serde_urlencoded;

pub type TrackerResult = Result<TrackerResponse, TrackerError>;

#[derive(Debug, Serialize)]
pub struct TrackerResponse {
    // Interval in seconds that the client should wait between sending regular requests to the
    // tracker.
    interval: u32,
    peers: Vec<Peer>,
}

#[derive(Debug, Serialize)]
pub struct TrackerError {
    failure: String,
}

impl TrackerError {
    fn new(msg: String) -> Self {
        Self { failure: msg }
    }
}

#[derive(Debug, Serialize)]
struct Peer {
    // peer's self selected ID
    #[serde(rename = "peer id")]
    peer_id: u32,
    // peer's Ipv4/6 address or DNS name
    ip: PeerAddress,
    port: u32,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum PeerAddress {
    Ip(IpAddr),
    Dns(String),
}

#[derive(Debug, Deserialize)]
struct TrackerRequest<'a> {
    // TODO: maybe there's a way to deserialize into [u8; 20] instead of &str?

    // 20-byte SHA1 hash of the value of the info key from the Metainfo file. Note that th value
    // will be a bencoded dictionary.
    info_hash: &'a [u8],  // byte array of length 20
    // 20-byte string used as a unique ID for the client, generated by the client at startup. This
    // is allowed to be any value, and may be binary data.
    peer_id: &'a [u8],  // byte array of length 20
    // The true address where the client is listening; if missing infer the ip address from the
    // address where the http request came from.
    ip: Option<IpAddr>,
    // Port number where the client is listening.
    port: u16,
    // Total number of bytes uploaded since the client sent the 'started' event to the tracker.
    uploaded: u32,
    // Total number of bytes downloaded since the client sent the 'started' event to the tracker.
    downloaded: u32,
    // The number of bytes the client still has left to download to get all included files.
    left: u32,
    event: Option<ClientEvent>,
    // The number of peers that the client would like to receive from the tracker.
    numwant: Option<u32>,
}

impl<'a> TrackerRequest<'a> {
    fn from_query_string<'b: 'a>(qs: &'b str) -> Result<Self, TrackerError> {
        serde_urlencoded::from_str(qs).map_err(|err| {
            TrackerError::new(err.to_string())
        })
    }

    fn validate_request(&self) -> Result<(), TrackerError> {
        let ret = match (self.info_hash.len(), self.peer_id.len()) {
            (20, 20) => Ok(()),
            (20, _) => Err("Invalid peerid: peerid is not 20 bytes long."),
            (_, _) => Err("Invalid infohash: infohash is not 20 bytes long."),
        };
        ret.map_err(|s: &str| TrackerError::new(s.to_string()))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ClientEvent {
    // The first request to the tracker must include the 'started' event.
    Started,
    // The client must send this event if the client is shutting down gracefully.
    Stopped,
    // This event must be sent to the tracker when the download completes.
    Completed,
}

pub struct Tracker;

impl Tracker {
    fn register_new_peer(req: &TrackerRequest) {
    }

    pub fn handle_session(req: Request<Body>, _opt: Arc<Opt>) -> TrackerResult {
        let uri = req.uri();
        let ret = match (req.method(), uri.path(), uri.query()) {
            (&Method::GET, "/announce", Some(query)) => {
                let qs = TrackerRequest::from_query_string(query)?;
                qs.validate_request()?;
                match qs.event {
                    Some(ClientEvent::Started) => Tracker::register_new_peer(&qs),
                    Some(ClientEvent::Stopped) => unimplemented!(),
                    Some(ClientEvent::Completed) => unimplemented!(),
                    None => unimplemented!(),
                }
                unimplemented!();
            },
            (&Method::GET, "/announce", None) => Err("Invalid request: no query string."),
            (&Method::GET, _, _) => Err("Unrecognized path, try '/announce'."),
            _ => Err("Invalid request type: client request was not an HTTP GET."),
        };

        ret.map_err(|s: &str| TrackerError::new(s.to_string()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::net::Ipv4Addr;
    use std::path::PathBuf;
    use serde_bencode;

    #[test]
    fn basic_ok_test() {
        let peer = Peer {
            peer_id: 1,
            ip: PeerAddress::Ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
            port: 6981,
        };
        let response = TrackerResponse {
            interval: 10,
            peers: vec![peer],
        };

        assert_eq!(
            serde_bencode::to_string(&response).unwrap(),
            "d8:intervali10e5:peersld2:ip9:127.0.0.17:peer idi1e4:porti6981eeee"
        );
    }

    #[test]
    fn basic_err_test() {
        let err = TrackerError {
            failure: "oops".to_string(),
        };

        assert_eq!(
            serde_bencode::to_string(&err).unwrap(),
            "d7:failure4:oopse"
        );
    }

    #[test]
    fn basic_handle_session() {
        // TODO: flesh this out
        let mut req = Request::builder()
            .uri("http://localhost:6981?info_hash=abcdefghijklmnopqrst&peer_id=abcdefghijklmnopqrst&ip=192.168.0.1&port=1000&uploaded=42&downloaded=10&left=20");
        let opt = Arc::new(Opt {
            root: PathBuf::new(),
            peers: 10,
        });

        Tracker::handle_session(req.body(Body::empty()).unwrap(), opt);
    }
}
