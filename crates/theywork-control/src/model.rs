use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use theywork_core::{ThreadIdentity, WorkerRole};

/// One configuration directory is bound to one canonical Codex source home.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlConfig {
    pub state_dir: PathBuf,
    pub codex_home: PathBuf,
    pub codex_program: PathBuf,
    /// Usually ["app-server"]. Explicit arguments also allow isolated fixtures.
    pub codex_args: Vec<String>,
    pub rpc_timeout_ms: u64,
}

impl ControlConfig {
    pub fn new(state_dir: impl Into<PathBuf>, codex_home: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
            codex_home: codex_home.into(),
            codex_program: PathBuf::from("codex"),
            codex_args: vec!["app-server".into()],
            rpc_timeout_ms: 15_000,
        }
    }

    pub(crate) fn normalize(&self) -> anyhow::Result<Self> {
        let mut value = self.clone();
        value.codex_home = std::fs::canonicalize(&self.codex_home)?;
        anyhow::ensure!(
            value.codex_home.is_dir(),
            "Codex source home is not a directory"
        );
        value.state_dir = absolute(&self.state_dir)?;
        value.rpc_timeout_ms = self.rpc_timeout_ms.clamp(100, 60_000);
        Ok(value)
    }
}

fn absolute(path: &Path) -> anyhow::Result<PathBuf> {
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    })
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capabilities {
    pub send: bool,
    pub steer: bool,
    pub interrupt: bool,
    pub reply: bool,
    pub attach: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedThread {
    pub identity: ThreadIdentity,
    #[serde(default)]
    pub role: WorkerRole,
    /// Durable ownership proof. Observed child notifications do not grant it.
    #[serde(default)]
    pub managed: bool,
    pub project: PathBuf,
    pub title: String,
    pub active_turn_id: Option<String>,
    pub status: String,
    pub latest_text: String,
    pub capabilities: Capabilities,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlEvent {
    pub sequence: u64,
    pub at: i64,
    pub method: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRequest {
    /// Host generation plus native JSON-RPC id; never reuse across connections.
    pub id: String,
    pub native_id: Value,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub method: String,
    pub params: Value,
    pub received_at: i64,
    /// A written reply remains visible until the provider confirms resolution.
    pub reply_sent: bool,
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperationStatus {
    Sending,
    Confirmed,
    Rejected,
    Uncertain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationReceipt {
    pub id: String,
    pub status: OperationStatus,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub detail: String,
    #[serde(default)]
    pub request_key: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlSnapshot {
    pub generation: String,
    pub connection_generation: String,
    pub codex_home: PathBuf,
    pub connected: bool,
    pub provider_info: Value,
    pub threads: BTreeMap<String, ManagedThread>,
    pub pending_requests: Vec<PendingRequest>,
    pub events: Vec<ControlEvent>,
    pub operations: BTreeMap<String, OperationReceipt>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum Request {
    Snapshot,
    Start {
        project: PathBuf,
        prompt: String,
        operation_id: String,
    },
    Send {
        thread_id: String,
        prompt: String,
        expected_turn_id: Option<String>,
        operation_id: String,
    },
    Interrupt {
        thread_id: String,
        turn_id: String,
        operation_id: String,
    },
    Reply {
        request_id: String,
        response: Value,
        operation_id: String,
    },
    /// Explicitly reopen only a thread already owned by this supervisor.
    Reconnect {
        thread_id: String,
        operation_id: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Envelope {
    pub token: String,
    pub request: Request,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Response {
    pub value: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Endpoint {
    pub address: std::net::SocketAddr,
    pub token: String,
    pub generation: String,
    pub codex_home: PathBuf,
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
