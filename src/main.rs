use std::collections::HashMap;
use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::time::SystemTime;
use serde_json::Value::Bool;
use serde::{Serialize, Deserialize};
use tokio::time;
use crate::EVSEStatus::Occupied;
use elasticsearch::{Elasticsearch, auth::Credentials, Error, http::transport::Transport, UpdateByQuery, Update, UpdateParts, CreateParts, BulkUpdateOperation, BulkOperation, BulkParts};
use elasticsearch::http::Method;
use elasticsearch::http::Method::Post;
use elasticsearch::http::request::JsonBody;
use elasticsearch::ingest::IngestPutPipelineParts;
use elasticsearch::params::OpType::Create;
use hyper::HeaderMap;
use serde_json::{json, Value};

// use tower_http::cors::{Any, CorsLayer};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "PascalCase")]
enum EVSEStatus {
    Available,
    Occupied,
    OutOfService,
    Unknown
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct EVSEStatusResponse {
    #[serde(rename = "EVSEStatuses")]
    evse_statuses: Vec<EVSEStatusRecordContainer>
}

#[derive(Serialize, Deserialize, Debug)]
struct EVSEStatusRecordContainer {
    #[serde(rename = "EVSEStatusRecord")]
    evse_status_record: Vec<EVSEStatusRecord>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct EVSEStatusRecord{
    #[serde(rename = "EvseID")]
    evse_id: String,
    #[serde(rename = "EVSEStatus")]
    evse_status: EVSEStatus
}

// Elastic Interface!
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Charging {
    start: SystemTime,
    end: Option<SystemTime>,
    nominal_max_power: f64,
    estimated_power: Option<f64>,
    charger: String,
    canton: Option<String>,
    zip: String,
    location: [f64;2],
}

impl Charging {
    pub fn set_end(&mut self, end: SystemTime) {
        self.end = Some(end);
    }
}

// Elastic Interface!
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Realtime {
    last_update: SystemTime,
    occupied: bool,
    nominal_max_power: f64,
    estimated_power: Option<f64>,
    canton: Option<String>,
    zip: String,
    location: [f64;2]
}

// kanton, plz, koordinaten

// TODO: Kanton form Coordinates


#[tokio::main]
async fn main() {

    let transport = Transport::cloud(cloud_id, credentials).unwrap();
    let client = Elasticsearch::new(transport);
    println!("Client: {:?}", client);

    let mut ops: Vec<BulkOperation<Value>> = Vec::new();
    ops.push(BulkOperation::update("3a", json!({
        "doc": {
            "message": "tweet updated"
        },
        "doc_as_upsert": true
    })).into());

    let res = client.bulk(BulkParts::Index("tweets"))
        .body(ops)
        .send()
        .await
        .unwrap();

    println!("{:?}", res);

    let mut occupied: HashMap<String, Charging> = HashMap::new();

    loop {
        let mut realtime: HashMap<String, Realtime> = HashMap::new();

        let mut slots: HashMap<String, EVSEStatus> = HashMap::new();
        let mut newly_unoccupied: HashMap<String, Charging> = HashMap::new();

        let status_response = fetch_evse_status().await;
        for container in status_response.evse_statuses {
            for status in container.evse_status_record {
                realtime.insert(status.evse_id.clone(), Realtime {
                    last_update: SystemTime::now(),
                    occupied: status.evse_status == Occupied,
                    nominal_max_power: lookup_nominal_power(&status.evse_id),
                    estimated_power: lookup_estimated_power(&status.evse_id),
                    canton: lookup_canton(&status.evse_id),
                    zip: lookup_zip(&status.evse_id),
                    location: lookup_location(&status.evse_id),
                });

                if status.evse_status == Occupied {
                    if let Some(occ) = occupied.get(&status.evse_id) {
                        // do nothing, was already occupied
                    } else {
                        // newly occupied
                        occupied.insert(status.evse_id.clone(), Charging {
                            start: SystemTime::now(),
                            end: None,
                            nominal_max_power: lookup_nominal_power(&status.evse_id),
                            estimated_power: lookup_estimated_power(&status.evse_id),
                            charger: status.evse_id.clone(),
                            canton: lookup_canton(&status.evse_id),
                            zip: lookup_zip(&status.evse_id),
                            location: lookup_location(&status.evse_id)
                        });
                    }
                } else {
                    if let Some(occ) = occupied.get(&status.evse_id) {
                        let mut occ = occ.clone();
                        occ.set_end(SystemTime::now());
                        newly_unoccupied.insert(status.evse_id.clone(), (occ).clone());
                        occupied.remove(&status.evse_id);
                    } else {
                        // do nothing, was already unocupied
                    }
                }
                slots.insert(status.evse_id, status.evse_status);
            }
        }

        // let mut occupiedCount = 0;
        // for (key, value) in slots.into_iter() {
        //     if value == Occupied {
        //         occupiedCount += 1;
        //     }
        // }

        println!("Newly unoccupied: {:?}", newly_unoccupied);
        // println!("Realtime: {:?}", realtime);

        time::sleep(time::Duration::from_secs(2)).await
    }
}

fn lookup_location(p0: &String) -> [f64; 2] {
    return [42_f64, 42_f64];
}

fn lookup_zip(p0: &String) -> String {
    return String::from("3000");
}

fn lookup_canton(p0: &String) -> Option<String> {
    return Some(String::from("BE"));
}

fn lookup_estimated_power(p0: &String) -> Option<f64> {
    return Some(42_f64);
}

fn lookup_nominal_power(p0: &String) -> f64 {
    return 42_f64;
}

// async fn root() {
//     println!("entering root handler");
// }


// async fn fetch_evse_data() {
//     let uri = "https://data.geo.admin.ch/ch.bfe.ladestellen-elektromobilitaet/data/ch.bfe.ladestellen-elektromobilitaet.json";
//     let resp = reqwest::get(uri).await.unwrap().bytes().await;
//     println!("Response: {:?}", resp);
// }

async fn fetch_evse_status() -> EVSEStatusResponse {
    let uri = "https://data.geo.admin.ch/ch.bfe.ladestellen-elektromobilitaet/status/ch.bfe.ladestellen-elektromobilitaet.json";
    let resp = reqwest::get(uri).await.unwrap().json::<EVSEStatusResponse>().await.unwrap();
    resp
}