use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventV1 {
    _kind: String,
    _api_version: String,
    pub level: Level,
    #[serde(rename = "auditID")]
    pub audit_id: Uuid,
    pub stage: Stage,
    #[serde(rename = "requestURI")]
    pub request_uri: String,
    pub verb: String,
    pub user: UserInfo,
    pub impersonated_user: Option<UserInfo>,
    #[serde(rename = "sourceIPs")]
    pub source_ips: Option<Vec<std::net::IpAddr>>,
    pub user_agent: Option<String>,
    pub object_ref: Option<ObjectReference>,
    pub response_status: Option<ResponseStatus>,
    pub request_object: Option<Value>,
    pub response_object: Option<Value>,
    pub request_received_timestamp: DateTime<Utc>,
    pub stage_timestamp: DateTime<Utc>,
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub enum Level {
    None,
    Metadata,
    Request,
    RequestResponse,
}

#[derive(Debug, Deserialize, Clone)]
pub enum Stage {
    RequestReceived,
    ResponseStarted,
    ResponseComplete,
    Panic,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct UserInfo {
    pub groups: Vec<String>,
    pub uid: Option<String>,
    pub username: String,
    pub extra: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectReference {
    resource: Option<String>,
    namespace: Option<String>,
    name: Option<String>,
    uid: Option<Uuid>,
    _api_group: Option<String>,
    _api_version: Option<String>,
    _resource_version: Option<String>,
    subresource: Option<String>,
}

impl fmt::Display for ObjectReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(namespace) = &self.namespace {
            write!(f, "{namespace}/")?;
        }

        if let Some(resource) = &self.resource {
            write!(f, "{resource}/")?;
        }

        if let Some(name) = &self.name {
            write!(f, "{name}")?;
        }

        if let Some(subresource) = &self.subresource {
            write!(f, "/{subresource}")?;
        }

        if let Some(uid) = &self.uid {
            write!(f, " ({uid})")?;
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResponseStatus {
    _api_version: Option<String>,
    _code: i32,
    _details: Option<StatusDetails>,
    _kind: Option<String>,
    _message: Option<String>,
    _metadata: Option<ListMeta>,
    _reason: Option<String>,
    _status: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusDetails {
    _causes: Option<Vec<StatusCause>>,
    _group: Option<String>,
    _kind: Option<String>,
    _name: Option<String>,
    _retry_after_seconds: Option<isize>,
    _uid: Option<Uuid>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusCause {
    _field: Option<String>,
    _message: String,
    _reason: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ListMeta {
    #[serde(rename = "continue")]
    _cont: Option<String>,
    _remaining_item_count: Option<isize>,
    _resource_version: Option<String>,
}
