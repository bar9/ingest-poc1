use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Serialize, Deserialize, Debug)]
pub struct EVSEDataResponse {
    #[serde(rename = "EVSEData")]
    pub evse_data: Vec<EVSEDataRecordContainer>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EVSEDataRecordContainer {
    #[serde(rename = "EVSEDataRecord")]
    pub evse_data_record: Vec<EVSEDataRecord>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EVSEDataRecord {
    #[serde(rename = "GeoCoordinates")]
    pub geo_coordinates: Google,
    #[serde(rename = "lastUpdate")]
    pub last_update: Option<String>,
    #[serde(rename = "EvseID")]
    pub evse_id: String,
    #[serde(rename = "Address")]
    pub address: Address,
    #[serde(rename = "ChargingFacilities")]
    pub charging_facilities: Vec<Map<String, Value>>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Google {
    #[serde(rename = "Google")]
    pub google: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Address {
    pub postal_code: Option<String>
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum EVSEStatus {
    Available,
    Occupied,
    OutOfService,
    Unknown
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EVSEStatusResponse {
    #[serde(rename = "EVSEStatuses")]
    pub evse_statuses: Vec<EVSEStatusRecordContainer>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EVSEStatusRecordContainer {
    #[serde(rename = "EVSEStatusRecord")]
    pub evse_status_record: Vec<EVSEStatusRecord>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EVSEStatusRecord{
    #[serde(rename = "EvseID")]
    pub evse_id: String,
    #[serde(rename = "EVSEStatus")]
    pub evse_status: EVSEStatus
}

enum IntOrString {
    Int(i32),
    String(String),
}