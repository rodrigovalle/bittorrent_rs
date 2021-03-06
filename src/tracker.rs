use crate::Opt;

use hyper::{Body, Method, Request};
use serde::{de, ser, Deserialize, Serialize};
use rand::seq::IteratorRandom;
use serde_urlencoded;

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::net::IpAddr;
use std::str;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

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

// Hash is used to avoid duplicates
// Consider ignoring peer_id so that changing peer_id doesn't cause us to store duplicate ip/port
// combinations in the hashset of a torrent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Peer {
    // peer's self selected ID
    #[serde(rename = "peer id")]
    peer_id: PeerId,
    // peer's Ipv4/6 address or DNS name
    ip: IpAddr,
    port: u16,
}

// TODO: newtype can borrow from the deserializer as long as the deserializer is alive
// TODO: consider serde_bytes?
macro_rules! newtype_bytearray {
    ($newtype:ident, $len:expr) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        struct $newtype([u8; $len]);

        // by default serde_bencode will serialize/deserialize byte arrays as bencoded lists of
        // integers instead of bencoded byte arrays, so we need to implement these traits ourselves
        // to get the behavior required by the bittorrent spec
        //
        // I think this is because serde doesn't have a specialized Serialize/Deserialize impl for
        // [u8; N], it just uses a generic [T; N] that calls deserialize_tuple. It might be
        // possible to use serde_bencode with an &[u8], where its Deserialize trait calls
        // deserialize_bytes.
        impl<'a> Serialize for $newtype {
            fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_bytes(&self.0)
            }
        }

        impl<'de, 'a: 'de> Deserialize<'de> for $newtype {
            fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct BytesVisitor;

                impl<'de> de::Visitor<'de> for BytesVisitor {
                    type Value = [u8; $len];

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        write!(formatter, "a byte array of length {}", $len)
                    }

                    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
                        match Self::Value::try_from(value) {
                            Ok(ret) => Ok(ret),
                            _ => Err(E::invalid_length(value.len(), &self)),
                        }
                    }

                    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                        match Self::Value::try_from(value.as_bytes()) {
                            Ok(ret) => Ok(ret),
                            _ => Err(E::invalid_length(value.len(), &self)),
                        }
                    }
                }

                Ok($newtype(deserializer.deserialize_bytes(BytesVisitor)?))
            }
        }
    };
}

newtype_bytearray!(InfoHash, 20);
newtype_bytearray!(PeerId, 20);

#[derive(Debug, Deserialize)]
struct TrackerRequest {
    // 20-byte SHA1 hash of the value of the info key from the Metainfo file. Note that th value
    // will be a bencoded dictionary.
    info_hash: InfoHash,
    // 20-byte string used as a unique ID for the client, generated by the client at startup. This
    // is allowed to be any value, and may be binary data.
    peer_id: PeerId,
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

impl TrackerRequest {
    fn from_query_string(qs: &str) -> Result<Self, TrackerError> {
        serde_urlencoded::from_str(qs).map_err(|err| TrackerError::new(err.to_string()))
    }

    fn validate_request(&self) -> Result<(), TrackerError> {
        // let ret = match (self.info_hash.len(), self.peer_id.len()) {
        //     (20, 20) => Ok(()),
        //     (20, _) => Err("Invalid peerid: peerid is not 20 bytes long."),
        //     (_, _) => Err("Invalid infohash: infohash is not 20 bytes long."),
        // };
        // ret.map_err(|s: &str| TrackerError::new(s.to_string()))
        Ok(())
    }

    fn normalize_request(&mut self) {
        self.numwant = self.numwant.or(Some(50));
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

pub struct Tracker {
    opt: Opt,
    // TODO: replace with a concurrent hashmap for finer grained locking?
    torrents: Mutex<HashMap<InfoHash, HashSet<Peer>>>,
    complete_count: AtomicU32,
}

impl Tracker {
    pub fn new(opt: Opt) -> Self {
        Self {
            opt,
            torrents: Mutex::new(HashMap::new()),
            complete_count: AtomicU32::new(0),
        }
    }

    /// Registers a new peer as interested in a torrent if we don't already know about this peer.
    fn maybe_register_new_peer(&self, req: &TrackerRequest) {
        let mut torrents = self.torrents.lock().unwrap();
        let peer = Peer {
            peer_id: req.peer_id.clone(), // could probably have this be a borrow?
            ip: req.ip.unwrap(), // TODO: we might need to infer the client's IP
            port: req.port,
        };

        torrents
            .entry(req.info_hash.clone()) // we identify a torrent by its info_hash
            .or_insert(HashSet::new()) // create a mapping for new torrents
            .insert(peer); // track all the peers participating in this torrent
    }

    /// Pick `numwant` number of random peers, excluding the client making this request, from the
    /// torrent that the client is interested in.
    // TODO: exclude the requester from the peer list
    fn get_peers(&self, req: &TrackerRequest) -> Vec<Peer> {
        let torrents = self.torrents.lock().unwrap();
        let mut rng = rand::thread_rng();
        let peers = torrents
            .get(&req.info_hash)
            .map_or(vec![], |peers: &HashSet<Peer>| {
                // we can copy these out or return the MutexGuard
                // since these borrow from the `torrents` MutexGuard we are not allowed to return
                // references without also holding the lock.
                peers.iter().choose_multiple(&mut rng, req.numwant.unwrap() as usize)
            });

        peers.into_iter().copied().collect()
    }

    pub fn handle_session(&self, req: Request<Body>) -> TrackerResult {
        let uri = req.uri();
        let ret = match (req.method(), uri.path(), uri.query()) {
            (&Method::GET, "/announce", Some(query)) => {
                let mut qs = TrackerRequest::from_query_string(query)?;
                qs.validate_request()?;
                qs.normalize_request();
                self.maybe_register_new_peer(&qs);
                match qs.event {
                    Some(ClientEvent::Started) => unimplemented!(),
                    Some(ClientEvent::Stopped) => unimplemented!(),
                    Some(ClientEvent::Completed) => {
                        self.complete_count.fetch_add(1, Ordering::Relaxed);
                    },
                    None => {}
                }
                Ok(TrackerResponse {
                    interval: 1,
                    peers: self.get_peers(&qs),
                })
            }
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
    use serde_bencode;
    use std::net::Ipv4Addr;
    use std::path::PathBuf;

    #[test]
    fn peer_id_ser_test() {
        let hash: [u8; 20] = ['a' as u8; 20];
        let peer_id = PeerId(hash);

        assert_eq!(
            format!("20:{}", str::from_utf8(&hash).unwrap()),
            serde_bencode::to_string(&peer_id).unwrap(),
        );
    }

    #[test]
    fn peer_id_de_test() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct TestData {
            peer_id: PeerId,
        }

        let hash: [u8; 20] = ['a' as u8; 20];
        let test_data = TestData { peer_id: PeerId(hash) };

        // this is throwing an error because serde can't deserialize a byte array into `an array of
        // length 20`
        assert_eq!(
            PeerId(hash),
            serde_bencode::from_str::<PeerId>("20:aaaaaaaaaaaaaaaaaaaa").unwrap(),
        );

        assert_eq!(
            test_data,
            serde_urlencoded::from_str::<TestData>("peer_id=aaaaaaaaaaaaaaaaaaaa").unwrap(),
        );
    }

    #[test]
    fn peer_id_wrong_length() {
        assert!(serde_bencode::from_str::<PeerId>("3:abc").is_err());
    }

    #[test]
    fn basic_ok_test() {
        let peer = Peer {
            peer_id: PeerId("abcdefghijklmnopqrst".as_bytes().try_into().unwrap()),
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 6981,
        };
        let response = TrackerResponse {
            interval: 10,
            peers: vec![peer],
        };

        assert_eq!(
            serde_bencode::to_string(&response).unwrap(),
            "d8:intervali10e5:peersld2:ip9:127.0.0.17:peer id20:abcdefghijklmnopqrst4:porti6981eeee"
        );
    }

    #[test]
    fn basic_err_test() {
        let err = TrackerError {
            failure: "oops".to_string(),
        };

        assert_eq!(serde_bencode::to_string(&err).unwrap(), "d7:failure4:oopse");
    }

    #[test]
    fn basic_handle_session() {
        // TODO: flesh this out
        let mut req = Request::builder()
            .uri("http://localhost:6981?info_hash=abcdefghijklmnopqrst&peer_id=abcdefghijklmnopqrst&ip=192.168.0.1&port=1000&uploaded=42&downloaded=10&left=20");
        let opt = Opt {
            root: PathBuf::new(),
            peers: 10,
        };

        let tracker = Tracker::new(opt);
        tracker.handle_session(req.body(Body::empty()).unwrap());
    }
}
