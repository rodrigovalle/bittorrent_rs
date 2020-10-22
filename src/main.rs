//! Bittorrent tracker
mod metainfo;
mod tracker;
use tracker::Tracker;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use structopt::StructOpt;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};

use serde_bencode;

const ADDR: [u8; 4] = [127, 0, 0, 1];
const PORT: u16 = 6969;

#[derive(Debug, StructOpt, Clone)]
pub struct Opt {
    /// Pass in a file or directory to serve.
    #[structopt(long, parse(from_os_str))]
    root: PathBuf,

    /// The number of peers to respond with.
    #[structopt(long, default_value = "50")]
    peers: u32,
}

#[tokio::main]
async fn main() {
    let opt = Arc::new(Opt::from_args());
    let addr = SocketAddr::from((ADDR, PORT));

    // futures have to have 'static lifetimes, so they can only hold references to things owned
    // by the future itself
    // async blocks can play the role of a safe 'static-maker
    // The async block can await a future that operates on borrowed data, while still being
    // 'static overall and thus spawnable on a thread pool or other executor, (by holding the data
    // while the future executes?)

    // make_service_fn is called for each connection received
    // service_fn is called for each request in that connection
    let make_service = make_service_fn(|_conn| {
        // when a new connection appears, clone opt (whose lifetime is longer than that of the
        // closure) so the connection owns a copy
        //
        // the closure object is created on the stack so references to opt are still alive and able
        // to be cloned.
        //
        // we can't just move opt into this closure because we move opt into a brand new nested
        // closure that is constructed every time a new connection appears. calling this closure
        // more than once would mean we move at least twice.
        let opt = opt.clone();

        async {
            // this same closure object created here gets called for every request on a single
            // connection
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                // move opt into this closure contained within this async block, and service a
                // request on this connection

                // we need to clone this a second time so that the async block below can own its
                // own copy, otherwise we "leak" a reference to a local of this closure by returning
                // it in the future created by async.
                let opt = opt.clone();
                async {
                    let response = Tracker::handle_session(req, opt);
                    Ok::<_, Infallible>(Response::new(Body::from(serde_bencode::to_string(&response).unwrap())))
                }
            }))
        }
    });

    // bind and accept new connections
    let server = Server::bind(&addr).serve(make_service);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
