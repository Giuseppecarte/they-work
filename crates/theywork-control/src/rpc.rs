use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

use crate::{security::read_line_limited, ControlConfig};
use serde_json::{json, Value};

#[derive(Debug)]
pub(crate) enum RpcError {
    Rejected(String),
    Uncertain(String),
}

type Waiter = mpsc::Sender<Result<Value, RpcError>>;

pub(crate) struct Rpc {
    input: Mutex<ChildStdin>,
    child: Mutex<Child>,
    pending: Arc<Mutex<HashMap<u64, Waiter>>>,
    next_id: AtomicU64,
    timeout: Duration,
}

impl Rpc {
    pub(crate) fn launch(
        config: &ControlConfig,
        events: mpsc::Sender<Option<Value>>,
    ) -> anyhow::Result<Arc<Self>> {
        let mut child = Command::new(&config.codex_program)
            .args(&config.codex_args)
            .env("CODEX_HOME", &config.codex_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child.stdin.take().expect("piped stdin");
        let output = child.stdout.take().expect("piped stdout");
        let pending: Arc<Mutex<HashMap<u64, Waiter>>> = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = pending.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            while let Ok(Some(line)) = read_line_limited(&mut reader) {
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    break;
                };
                if message.get("method").is_none() {
                    if let Some(id) = message.get("id").and_then(Value::as_u64) {
                        if let Some(waiter) = reader_pending.lock().unwrap().remove(&id) {
                            let result = if let Some(error) = message.get("error") {
                                Err(RpcError::Rejected(error.to_string()))
                            } else {
                                Ok(message.get("result").cloned().unwrap_or(Value::Null))
                            };
                            let _ = waiter.send(result);
                        }
                    }
                } else if events.send(Some(message)).is_err() {
                    break;
                }
            }
            for (_, waiter) in reader_pending.lock().unwrap().drain() {
                let _ = waiter.send(Err(RpcError::Uncertain(
                    "Provider connection closed; do not resend".into(),
                )));
            }
            let _ = events.send(None);
        });
        Ok(Arc::new(Self {
            input: Mutex::new(input),
            child: Mutex::new(child),
            pending,
            next_id: AtomicU64::new(1),
            timeout: Duration::from_millis(config.rpc_timeout_ms),
        }))
    }

    pub(crate) fn initialize(&self) -> Result<Value, RpcError> {
        let value = self.call("initialize", json!({"clientInfo":{"name":"they_work","title":"They Work","version":env!("CARGO_PKG_VERSION")}}))?;
        self.write(&json!({"method":"initialized","params":{}}))?;
        Ok(value)
    }

    pub(crate) fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, sender);
        if let Err(error) = self.write(&json!({"id":id,"method":method,"params":params})) {
            self.pending.lock().unwrap().remove(&id);
            return Err(error);
        }
        match receiver.recv_timeout(self.timeout) {
            Ok(result) => result,
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(RpcError::Uncertain(
                    "Provider acknowledgement timed out; do not resend".into(),
                ))
            }
        }
    }

    pub(crate) fn reply(&self, id: &Value, result: Value) -> Result<(), RpcError> {
        self.write(&json!({"id":id,"result":result}))
    }

    fn write(&self, message: &Value) -> Result<(), RpcError> {
        let mut input = self.input.lock().unwrap();
        let result = (|| -> std::io::Result<()> {
            serde_json::to_writer(&mut *input, message)?;
            input.write_all(b"\n")?;
            input.flush()
        })();
        result.map_err(|error| {
            RpcError::Uncertain(format!("Provider write failed: {error}; do not resend"))
        })
    }
}

impl Drop for Rpc {
    fn drop(&mut self) {
        // Only the supervisor owns Rpc. A UI client never owns or kills it.
        if let Ok(child) = self.child.get_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
