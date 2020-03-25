use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::ops::Add;
use std::env;
use std::fs;

use rumq_client::{self, QoS, MqttOptions, Request, MqttEventLoop, eventloop};
use serde::{Serialize, Deserialize};
use jsonwebtoken::{encode, Algorithm, Header, EncodingKey};
use futures_util::stream::StreamExt;
use tokio::sync::mpsc::{channel, Sender};
use tokio::task;
use tokio::time;

// RUST_LOG=rumq_client=debug PROJECT=cloudlinc REGISTRY=iotcore cargo run --color=always --package rumq-client --example gcloud

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    color_backtrace::install();

    let (requests_tx, requests_rx) = channel(1);
    let mqttoptions = gcloud();
    let mut eventloop = eventloop(mqttoptions, requests_rx);

    task::spawn(async move {
        requests(requests_tx).await;
        time::delay_for(Duration::from_secs(100)).await;
    });

    stream_it(&mut eventloop).await;
}

async fn stream_it(eventloop: &mut MqttEventLoop) {
    let mut stream = eventloop.stream();

    while let Some(item) = stream.next().await {
        println!("Received = {:?}", item);
    }

    println!("Stream done");
}

async fn requests(mut requests_tx: Sender<Request>) {
    for i in 0..15 {
        requests_tx.send(publish_request(i)).await.expect("problem sending");
        time::delay_for(Duration::from_secs(1)).await; 
    }

    time::delay_for(Duration::from_secs(100)).await; 
}

fn gcloud() -> MqttOptions {
    let mut mqttoptions = MqttOptions::new(&id(), "mqtt.googleapis.com", 8883);
    mqttoptions.set_keep_alive(9);
    let password = gen_iotcore_password();
    let ca = fs::read("certs/roots.pem").expect("couldn't find the roots.pem");

    mqttoptions
        .set_ca(ca)
        .set_credentials("unused", &password);

    mqttoptions 
}

fn publish_request(i: u8) -> Request {
    let topic = "/devices/".to_owned() +  "arr_blah/events";
    let payload = vec![1, 2, 3, i];

    let publish = rumq_client::publish(&topic, QoS::AtLeastOnce, payload);
    Request::Publish(publish)
}

fn id() -> String {
    let project = env::var("PROJECT").expect("no project env set");
    let registry = env::var("REGISTRY").expect("no registry env set");

    "projects/".to_owned() + &project + "/locations/us-central1/registries/" + &registry + "/devices/" + "arr_blah"
}

fn gen_iotcore_password() -> String {
    let key = fs::read("certs/private_key").expect("couldn't find the private_key");
    let key = EncodingKey::from_rsa_pem(&key).expect("problem converting key from pem");

    let project = env::var("PROJECT").expect("again, no project env set");
    #[derive(Debug, Serialize, Deserialize)]
    struct Claims {
        iat: u64,
        exp: u64,
        aud: String,
    }

    let jwt_header = Header::new(Algorithm::RS256);
    let iat = SystemTime::now().duration_since(UNIX_EPOCH).expect("time is broken").as_secs();
    let exp = SystemTime::now().add(Duration::from_secs(300)).duration_since(UNIX_EPOCH).expect("time is still broken").as_secs();

    let claims = Claims { iat, exp, aud: project };
    encode(&jwt_header, &claims, &key).expect("failed to encode the jwt")
}
