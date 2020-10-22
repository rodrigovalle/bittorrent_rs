//! Bittorrent tracker
mod metainfo;
mod tracker;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;

use structopt::StructOpt;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};

use serde_bencode;

const ADDR: [u8; 4] = [127, 0, 0, 1];
const PORT: u16 = 6969;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Pass in a file or directory to serve.
    #[structopt(parse(from_os_str))]
    root: PathBuf,
}

async fn handle(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let resp = tracker::handle_session(req);
    Ok(Response::new(Body::from(serde_bencode::to_string(&resp).unwrap())))
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();

    let addr = SocketAddr::from((ADDR, PORT));
    let make_service = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(handle))
    });

    // TODO: Generate a .torrent metainfo file

    // bind and accept new connections
    let server = Server::bind(&addr).serve(make_service);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
