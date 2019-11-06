#[deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate serde_json;
use std::collections::HashMap;
use url::{Url, ParseError};
use std::sync::{RwLock, Arc};
use std::time::{Duration};
use ttl_cache::TtlCache;


use futures::{future, Future, Stream, Async};

use hyper::{Body, Client, Method, Request, Response,  StatusCode, header};
use hyper::client::HttpConnector;

static NOTFOUND: &[u8] = b"Not Found";
static INDEX: &[u8] = b"<a href=\"test.html\">test.html</a>";

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item=Response<Body>, Error=GenericError> + Send>;

struct BodyClone<T> {
    body: T,
    buffer: Option<Vec<u8>>,
    sender: Option<futures::sync::oneshot::Sender<Vec<u8>>>,
}

impl BodyClone<hyper::Body> {
    fn flush(&mut self) {
        if let (Some(buffer), Some(sender)) = (self.buffer.take(), self.sender.take()) {
            if sender.send(buffer).is_err() {}
        }
    }

    fn push(&mut self, chunk: &hyper::Chunk) {
        use hyper::body::Payload;

        let length = if let Some(buffer) = self.buffer.as_mut() {
            buffer.extend_from_slice(chunk);
            buffer.len() as u64
        } else {
            0
        };

        if let Some(content_length) = self.body.content_length() {
            if length >= content_length {
                self.flush();
            }
        }
    }
}

impl Stream for BodyClone<hyper::Body> {
    type Item = hyper::Chunk;
    type Error = hyper::Error;

    fn poll(&mut self) -> futures::Poll<Option<Self::Item>, Self::Error> {
        match self.body.poll() {
            Ok(Async::Ready(Some(chunk))) => {
                self.push(&chunk);
                Ok(Async::Ready(Some(chunk)))
            }
            Ok(Async::Ready(None)) => {
                self.flush();
                Ok(Async::Ready(None))
            }
            other => other,
        }
    }
}


pub type BufferFuture = Box<dyn Future<Item = Vec<u8>, Error = ()> + Send>;

trait CloneBody {
    fn clone_body(self) -> (hyper::Body, BufferFuture);
}

impl CloneBody for hyper::Body {
    fn clone_body(self) -> (hyper::Body, BufferFuture) {
        let (sender, receiver) = futures::sync::oneshot::channel();

        let cloning_stream = BodyClone {
            body: self,
            buffer: Some(Vec::new()),
            sender: Some(sender),
        };

        (
            hyper::Body::wrap_stream(cloning_stream),
            Box::new(receiver.map_err(|_| ())),
        )
    }
}


fn _get_headers(req: &hyper::Response<hyper::Body>) -> HashMap<String,String> {
    
    let mut header_hashmap = HashMap::new();
    for (k, v) in req.headers() {
        let k = k.as_str().to_owned();
        let v = String::from_utf8_lossy(v.as_bytes()).into_owned();
        header_hashmap.insert(k, v);   
    }
    header_hashmap
}

fn get_qs(req: &Request<Body>) -> Result<(HashMap<String,String>), ParseError> {
    let uri_string = format!("{}{}", "http://127.0.0.1:1337",req.uri().to_string());

    let url = Url::parse(&uri_string)?;
    let map_str: HashMap<String,String> = url.query_pairs().into_owned().collect();
    Ok(map_str)
    
}


fn proxy_res(req: Request<Body>, kvs: & 'static  Arc<RwLock<TtlCache<String, String>>>, client: &Client<HttpConnector>) -> ResponseFuture {

    let qs = get_qs(&req).unwrap();
    let q = qs.get("q").unwrap();
    let url: &str = &format!("http://localhost:1337/res/loader?q={}", q);
    let kv_clone = kvs.clone();
    
    let data = kv_clone.read().expect("RwLock poisoned");
    let has_key = data.contains_key(url);
    if has_key {
     //   println!("HIT");    
        let val  = data.get(url);
        serve_from_cache(&val.unwrap())
    } else {
        
        println!("MISS");
        fetch_data(url, kvs, client)
    }
    
   
}


fn fetch_data (url: &str ,_kvs: & 'static Arc<RwLock<TtlCache<String, String>>>, client: &Client<HttpConnector>) -> ResponseFuture {

    let kln = Arc::clone( _kvs);
    let in_url = url.to_string();
    let rslt = client.get(url.parse().unwrap()).from_err().map(move |web_res| {

       // let headers = get_headers(&web_res);
        let into_body =web_res.into_body();
        let (cloned_body, buffer)  =  into_body.clone_body();
        
        println!("fetching ... ");

        let _complete_future = buffer.then(move | xx| -> Result<_, ()>{
            println!("fetching ... complete ");
            let mut wkvs = kln.write().expect("RwLock poisoned");
            let buf = xx.unwrap();
            let str_buffer  = std::str::from_utf8(&buf).unwrap();
            wkvs.insert(in_url, str_buffer.to_string(), Duration::from_secs(30));
            Ok(())
        });
        
        hyper::rt::spawn(_complete_future);


        println!("fetching ... streaming start ");
        Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(cloned_body)
                .unwrap()
    });
    Box::new(rslt)
}

fn serve_from_cache(val: &str) -> ResponseFuture{
    let r = Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from( val.to_string()))
                .unwrap();
            Box::new(future::ok(r))
}



pub fn match_route(req: Request<Body>, client: &Client<HttpConnector>, kvs: & 'static Arc<RwLock<TtlCache<String, String>>>) -> ResponseFuture {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            let body = Body::from(INDEX);
            Box::new(future::ok(Response::new(body)))
        }

        (&Method::GET, "/res/loader") => {
            proxy_res(req, & kvs, client)
        }
        _ => {
            // Return 404 not found response.
            let body = Body::from(NOTFOUND);
            Box::new(future::ok(Response::builder()
                                         .status(StatusCode::NOT_FOUND)
                                         .body(body)
                                         .unwrap()))
        }
    }
}