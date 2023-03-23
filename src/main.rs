use std::collections::HashMap;
use axum::{routing::get, Router};
use std::net::SocketAddr;
use serde_json::Value::Bool;
use serde::{Serialize, Deserialize};

// use tower_http::cors::{Any, CorsLayer};

#[derive(Serialize, Deserialize, Debug)]
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

#[tokio::main]
async fn main() {
    let mut occupiedSlots: HashMap<String, EVSEStatus> = HashMap::new();

    fetch_evse_status().await;
}

// async fn root() {
//     println!("entering root handler");
// }


// async fn fetch_evse_data() {
//     let uri = "https://data.geo.admin.ch/ch.bfe.ladestellen-elektromobilitaet/data/ch.bfe.ladestellen-elektromobilitaet.json";
//     let resp = reqwest::get(uri).await.unwrap().bytes().await;
//     println!("Response: {:?}", resp);
// }

async fn fetch_evse_status() {
    let uri = "https://data.geo.admin.ch/ch.bfe.ladestellen-elektromobilitaet/status/ch.bfe.ladestellen-elektromobilitaet.json";
    let resp = reqwest::get(uri).await.unwrap().json::<EVSEStatusResponse>().await.unwrap();
    println!("Response: {:?}", resp);
}