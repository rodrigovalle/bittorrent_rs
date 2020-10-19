use std::net::{IpAddr, Ipv4Addr};
use std::collections::HashMap;
use hyper::{Body, Method, Request};
use serde::{Serialize, Deserialize};
use serde_urlencoded;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum TrackerResult {
    Response {
        interval: u32,
        peers: Vec<Peer>,
    },
    Error {
        failure: &'static str,
    },
}

#[derive(Debug, Serialize)]
struct Peer {
    #[serde(rename = "peer id")]
    peer_id: u32,
    ip: IpAddr,
    port: u32,
}

#[derive(Debug, Deserialize)]
struct TrackerRequest<'a> {
    // TODO: maybe there's a way to deserialize into [u8; 20] instead of &str?
    info_hash: &'a str,  // byte array of length 20
    peer_id: &'a str,  // byte array of length 20
    ip: IpAddr,
    port: u16,
    uploaded: u32,
    downloaded: u32,
    left: u32,
    event: Option<ClientEvent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ClientEvent {
    Started,
    Completed,
    Stopped,
}

pub fn handle_session(req: Request<Body>) -> TrackerResult {
    let uri = req.uri();
    match (req.method(), uri.path(), uri.query()) {
        (&Method::GET, path, Some(query)) => {
            let qs: TrackerRequest = serde_urlencoded::from_str(query).unwrap();
            println!("{:?}  {:?}", path, qs);
            TrackerResult::Response {
                interval: 10,
                peers: vec![],
            }
        },
        _ => {
            TrackerResult::Error {
                failure: "Needs a GET request with a path and a query string",
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_bencode;

    #[test]
    fn basic_ok_test() {
        let peer = Peer {
            peer_id: 1,
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 6981,
        };
        let response = TrackerResult::Response {
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
        let err = TrackerResult::Error {
            failure: "oops",
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

        handle_session(req.body(Body::empty()).unwrap());
    }
}
