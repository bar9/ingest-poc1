use std::alloc::System;
use std::cmp::max;
use std::collections::HashMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use std::str::FromStr;

use elasticsearch::{auth::Credentials, BulkOperation, BulkParts, Elasticsearch, http::transport::Transport};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::time;
use uuid::Uuid;

use dotenv::dotenv;

mod input_models;
mod output_models;
use crate::input_models::*;
use crate::input_models::EVSEStatus::Occupied;
use crate::output_models::*;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let cloud_id= env::var("ELASTIC_ID").unwrap();
    let credentials = Credentials::Basic(env::var("ELASTIC_USERNAME").unwrap(), env::var("ELASTIC_PASSWORD").unwrap());
    let transport = Transport::cloud(&cloud_id, credentials).unwrap();
    let client = Elasticsearch::new(transport);

    let mut occupied: HashMap<String, Charging> = HashMap::new();

    let mut lookup_table: HashMap<String, LookupTableEntry> = HashMap::new();

    async fn refresh_lookup_table(lookup_table: &mut HashMap<String, LookupTableEntry>) {
        let data_response = fetch_evse_data().await;
        for container in data_response.evse_data {
            for record in container.evse_data_record {
                let coords_str: Vec<&str> = record.geo_coordinates.google.split(' ').collect();
                let coords_flt = [coords_str[0].parse::<f64>().unwrap(), coords_str[1].parse::<f64>().unwrap()];
                let max_nominal_power = record.charging_facilities.into_iter()
                    .fold(0_f64, |prev, next|
                        if let Some(x) = next.get("power") {
                            x.as_f64().unwrap_or(f64::from_str(x.as_str().unwrap_or("")).unwrap_or(11_f64))
                        } else { 11_f64 });
                let estimated_power = 0.6 * if max_nominal_power > 11_f64 {max_nominal_power} else {11_f64};

                let lte = LookupTableEntry{
                    location: coords_flt,
                    zip: record.address.postal_code.unwrap_or(String::from("-")),
                    canton: "BE".to_string(),
                    estimated_power: estimated_power,
                    max_nominal_power: max_nominal_power
                };
                lookup_table.insert(record.evse_id, lte);
            }
        }
    }

    refresh_lookup_table(&mut lookup_table).await;

    loop {
        let mut realtime: HashMap<String, Realtime> = HashMap::new();

        let mut slots: HashMap<String, EVSEStatus> = HashMap::new();
        let mut newly_unoccupied: HashMap<String, Charging> = HashMap::new();

        let status_response = fetch_evse_status().await;
        for container in status_response.evse_statuses {
            for status in container.evse_status_record {
                let lookup_entry = lookup_table.get(&status.evse_id).unwrap();
                realtime.insert(status.evse_id.clone(), Realtime {
                    last_update: now(),
                    occupied: status.evse_status == Occupied,
                    nominal_max_power: lookup_entry.max_nominal_power,
                    estimated_power: Some(lookup_entry.estimated_power.clone()),
                    zip: lookup_entry.zip.clone(),
                    location: lookup_entry.location
                });

                if status.evse_status == Occupied {
                    if let Some(_) = occupied.get(&status.evse_id) {
                        // do nothing, was already occupied
                    } else {
                        // newly occupied
                        occupied.insert(status.evse_id.clone(), Charging {
                            start: now(),
                            end: None,
                            nominal_max_power: lookup_entry.max_nominal_power,
                            estimated_power: Some(lookup_entry.estimated_power.clone()),
                            zip: lookup_entry.zip.clone(),
                            location: lookup_entry.location,
                            charger: status.evse_id.clone(),
                            energy: Some(0_f64),
                        });
                    }
                } else {
                    if let Some(occ) = occupied.get(&status.evse_id) {
                        let mut occ = occ.clone();
                        occ.set_end(now());
                        newly_unoccupied.insert(status.evse_id.clone(), (occ).clone());
                        occupied.remove(&status.evse_id);
                    } else {
                        // do nothing, was already unocupied
                    }
                }
                slots.insert(status.evse_id, status.evse_status);
            }
        }

        // println!("Newly unoccupied: {:?}", newly_unoccupied);

        let mut ops: Vec<BulkOperation<Value>> = Vec::new();
        for (index, no) in &newly_unoccupied {
            let mut update_map: HashMap<String, Value> = HashMap::new();
            update_map.insert(String::from("doc"), serde_json::to_value(&no).unwrap());
            update_map.insert(String::from("doc_as_upsert"), json!(true));
            let id = Uuid::new_v4();
            ops.push(BulkOperation::from(BulkOperation::update(id.to_string(), serde_json::to_value(&update_map).unwrap())))
        }

        if &newly_unoccupied.len() > &0 {
            let res = client.bulk(BulkParts::Index("charging"))
                .body(ops)
                .send()
                .await
                .unwrap();

            println!("Charging bulk updated: {:?}", res);
        }

        let mut rt_ops: Vec<BulkOperation<Value>> = Vec::new();
        println!("{:?}", realtime);

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

        println!("Realtime bulk updated: {:?}", res);

        time::sleep(time::Duration::from_secs(5)).await
    }
}

struct LookupTableEntry {
    location: [f64; 2],
    zip: String,
    canton: String,
    estimated_power: f64,
    max_nominal_power: f64
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

fn now() -> u128 {
    let now = SystemTime::now();
    now.duration_since(UNIX_EPOCH).unwrap().as_millis()
}