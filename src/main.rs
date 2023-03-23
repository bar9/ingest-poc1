use std::collections::HashMap;
use std::env;
use std::time::SystemTime;

use elasticsearch::{auth::Credentials, BulkOperation, BulkParts, Elasticsearch, http::transport::Transport};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::time;

use dotenv::dotenv;

use crate::EVSEStatus::Occupied;

#[derive(Serialize, Deserialize, Debug)]
struct EVSEDataResponse {
    #[serde(rename = "EVSEData")]
    evse_data: Vec<EVSEDataRecordContainer>
}

#[derive(Serialize, Deserialize, Debug)]
struct EVSEDataRecordContainer {
    #[serde(rename = "EVSEDataRecord")]
    evse_data_record: Vec<EVSEDataRecord>,
}

#[derive(Serialize, Deserialize, Debug)]
struct EVSEDataRecord {
    #[serde(rename = "GeoCoordinates")]
    geo_coordinates: Google,
    #[serde(rename = "lastUpdate")]
    last_update: Option<String>, //ISODate
    #[serde(rename = "EvseID")]
    evse_id: String,
    #[serde(rename = "Address")]
    address: Address,
    #[serde(rename = "ChargingFacilities")]
    charging_facilities: Vec<Map<String, Value>>
}

#[derive(Serialize, Deserialize, Debug)]
struct Google {
    #[serde(rename = "Google")]
    google: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Address {
    postal_code: Option<String>
}

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
    nominal_max_power: f64, //grösste
    estimated_power: Option<f64>, //immer min. 11
    canton: Option<String>,
    zip: String,
    location: [f64;2]
}

enum IntOrString {
    Int(i32),
    String(String),
}

#[tokio::main]
async fn main() {
    dotenv().ok();


    // let elastic_endpoint = env::var("ELASTIC_ENDPOINT").unwrap();
    let cloud_id= env::var("ELASTIC_ID").unwrap();
    let credentials = Credentials::Basic(env::var("ELASTIC_USERNAME").unwrap(), env::var("ELASTIC_PASSWORD").unwrap());
    let transport = Transport::cloud(&cloud_id, credentials).unwrap();
    let client = Elasticsearch::new(transport);

    let mut occupied: HashMap<String, Charging> = HashMap::new();

    let data_response = fetch_evse_data().await;
    for container in data_response.evse_data {
        for record in container.evse_data_record {

        }
    }

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
                    if let Some(_) = occupied.get(&status.evse_id) {
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

        println!("Newly unoccupied: {:?}", newly_unoccupied);

        let mut ops: Vec<BulkOperation<Value>> = Vec::new();
        for (index, no) in &newly_unoccupied {
            let mut update_map: HashMap<String, Value> = HashMap::new();
            update_map.insert(String::from("doc"), serde_json::to_value(&no).unwrap());
            update_map.insert(String::from("doc_as_upsert"), json!(true));
            ops.push(BulkOperation::from(BulkOperation::update(index.clone(), serde_json::to_value(&update_map).unwrap())))
        }

        if &newly_unoccupied.len() > &0 {
            let res = client.bulk(BulkParts::Index("charging"))
                .body(ops)
                .send()
                .await
                .unwrap();

            println!("{:?}", res);
        }

        let mut rt_ops: Vec<BulkOperation<Value>> = Vec::new();
        for (index, rt) in &realtime{
            let mut update_map: HashMap<String, Value> = HashMap::new();
            update_map.insert(String::from("doc"), serde_json::to_value(&rt).unwrap());
            update_map.insert(String::from("doc_as_upsert"), json!(true));
            rt_ops.push(BulkOperation::from(BulkOperation::update(index.clone(), serde_json::to_value(&update_map).unwrap())))
        }

        let res = client.bulk(BulkParts::Index("realtime"))
            .body(rt_ops)
            .send()
            .await
            .unwrap();

        println!("{:?}", res);



        println!("{:?}", occupied);
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

async fn fetch_evse_data() -> EVSEDataResponse {
    let uri = "https://data.geo.admin.ch/ch.bfe.ladestellen-elektromobilitaet/data/ch.bfe.ladestellen-elektromobilitaet.json";
    let resp = reqwest::get(uri).await.unwrap().json::<EVSEDataResponse>().await.unwrap();
    resp
}

async fn fetch_evse_status() -> EVSEStatusResponse {
    let uri = "https://data.geo.admin.ch/ch.bfe.ladestellen-elektromobilitaet/status/ch.bfe.ladestellen-elektromobilitaet.json";
    let resp = reqwest::get(uri).await.unwrap().json::<EVSEStatusResponse>().await.unwrap();
    resp
}