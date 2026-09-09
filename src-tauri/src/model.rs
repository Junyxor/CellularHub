use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Mock,
    At,
    WindowsMbn,
    Android,
    Router,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySet {
    pub data: bool,
    pub sms_receive: bool,
    pub sms_send: bool,
    pub esim: bool,
    pub signal: bool,
    pub operator: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub backend: BackendKind,
    pub port: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub capabilities: CapabilitySet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellularSnapshot {
    pub backend: BackendKind,
    pub device_name: String,
    pub operator_name: String,
    pub network_type: String,
    pub signal_percent: u8,
    pub rssi_dbm: Option<i32>,
    pub connected: bool,
    pub sms_ready: bool,
    pub esim_ready: bool,
    pub last_error: Option<String>,
}

impl Default for CellularSnapshot {
    fn default() -> Self {
        Self {
            backend: BackendKind::Mock,
            device_name: "未连接设备".into(),
            operator_name: "等待设备".into(),
            network_type: "—".into(),
            signal_percent: 0,
            rssi_dbm: None,
            connected: false,
            sms_ready: false,
            esim_ready: false,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmsMessage {
    pub id: String,
    pub sender: String,
    pub body: String,
    pub received_at: String,
    pub unread: bool,
    #[serde(default)]
    pub archived: bool,
    pub backend: BackendKind,
    pub slot: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMeta {
    pub sender: String,
    pub alias: String,
    pub notes: String,
    pub pinned: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtConnectRequest {
    pub port: String,
    pub baud_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EsimCustomField {
    pub id: String,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EsimProfile {
    pub id: String,
    pub display_name: String,
    pub country_code: String,
    pub country_name: String,
    pub operator_name: String,
    pub phone_number: String,
    pub iccid: String,
    pub plan_name: String,
    pub retention_cost: f64,
    pub currency: String,
    pub billing_cycle: String,
    pub expiry_date: Option<String>,
    pub renewal_date: Option<String>,
    pub auto_renew: bool,
    pub status: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub custom_fields: Vec<EsimCustomField>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EsimCompatCandidate {
    pub port: String,
    pub at_apdu: bool,
    pub at_csim: bool,
    pub vendor_esim: bool,
    pub eid: Option<String>,
    pub lpac_available: bool,
    pub install_ready: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EsimCompatReport {
    pub windows_lpa_available: bool,
    pub lpac_path: Option<String>,
    pub candidates: Vec<EsimCompatCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EsimCompatInstallRequest {
    pub port: String,
    pub activation_code: String,
    pub transport: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckItem {
    pub id: String,
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseDiagnostics {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub data_dir: String,
    pub message_count: usize,
    pub unread_count: usize,
    pub esim_profile_count: usize,
    pub conversation_meta_count: usize,
    pub snapshot: CellularSnapshot,
    pub device_count: usize,
    pub checks: Vec<HealthCheckItem>,
    pub generated_at: String,
}
