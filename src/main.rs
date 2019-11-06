#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate serde_json;

use crate::futures::Future;
use futures::{future};
use hyper::{Client,Server};
use hyper::service::service_fn;

use ttl_cache::TtlCache;

use std::sync::{RwLock,Arc};
mod proxy;
#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref KVS: Arc<RwLock<TtlCache<String,String>>> = {
        
      let  map: TtlCache<String,String>= TtlCache::new(10);
      let kvs = Arc::new(RwLock::new(map));
      kvs
    };
}


fn main() {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1337".parse().unwrap();
    
    hyper::rt::run(future::lazy(move || {
        // Share a `Client` with all `Service`s
        let client = Client::new();

        let new_service = move || {
            // Move a clone of `client` into the `service_fn`.
            let client = client.clone();
            
            service_fn(move |req| {
                
                proxy::handlers::match_route(req, &client, &KVS)
            })
        };

        let server = Server::bind(&addr)
            .serve(new_service)
            .map_err(|e| eprintln!("server error: {}", e));

        println!("Listening on http://{}", addr);

        server
    }));
}