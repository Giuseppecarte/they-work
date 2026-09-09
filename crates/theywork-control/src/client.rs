use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::model::*;
use crate::security;
use crate::state_storage::StateStorage;

/// Cheap cloneable connection settings. RPC methods block; call them on the
/// UI's I/O worker. snapshot uses a bounded 500ms timeout and no provider RPC.
#[derive(Debug, Clone)]
pub struct ControlClient {
    config: ControlConfig,
}

impl ControlClient {
    /// Read durable ownership without starting a host or contacting a provider.
    /// Capabilities and pending prompts are invalid until explicit reconnect.
    pub fn saved_snapshot(config: ControlConfig) -> Result<ControlSnapshot> {
        Self::saved_snapshot_optional(config)?.context("No saved control state exists")
    }

    /// A genuinely unused control directory returns None. Established state
    /// loss, inaccessible storage and invalid data remain actionable errors.
    pub fn saved_snapshot_optional(config: ControlConfig) -> Result<Option<ControlSnapshot>> {
        let config = config.normalize()?;
        let Some(mut snapshot) = StateStorage::read_snapshot(&config.state_dir)? else {
            return Ok(None);
        };
        anyhow::ensure!(
            snapshot.codex_home == config.codex_home,
            "Saved control source home mismatch"
        );
        snapshot.connected = false;
        snapshot.pending_requests.clear();
        for thread in snapshot.threads.values_mut() {
            thread.capabilities = Capabilities::default();
            thread.active_turn_id = None;
            thread.status = "disconnected".into();
        }
        Ok(Some(snapshot))
    }

    pub fn connect(config: ControlConfig) -> Result<Self> {
        let client = Self {
            config: config.normalize()?,
        };
        client.snapshot()?;
        Ok(client)
    }

    pub fn connect_or_spawn(config: ControlConfig, executable: impl AsRef<Path>) -> Result<Self> {
        let config = config.normalize()?;
        security::private_dir(&config.state_dir)?;
        let client = Self {
            config: config.clone(),
        };
        if client.snapshot().is_ok() {
            return Ok(client);
        }
        // The startup lock prevents two clients from racing to replace config.
        let startup = security::private_open(&config.state_dir.join("startup.lock"), true)?;
        startup.lock()?;
        if client.snapshot().is_ok() {
            return Ok(client);
        }
        // Surface established-state loss before spawning a detached process
        // whose startup error would otherwise be reduced to an exit code.
        StateStorage::read_snapshot(&config.state_dir)?;
        let config_path = config.state_dir.join("config.json");
        if let Some(bytes) = security::read_private_optional(&config_path)? {
            let prior: ControlConfig = serde_json::from_slice(&bytes)?;
            anyhow::ensure!(
                prior.codex_home == config.codex_home,
                "Control directory belongs to a different source home"
            );
        }
        security::write_private(&config_path, &serde_json::to_vec(&config)?)?;
        let mut command = Command::new(executable.as_ref());
        command
            .arg("--control-host")
            .arg(&config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: setsid is async-signal-safe and touches no Rust state in
            // the child between fork and exec. Stdio is detached above.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0000_0008 | 0x0000_0200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        }
        let mut child = command
            .spawn()
            .context("Could not launch the local control host")?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if client.snapshot().is_ok() {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(client);
            }
            if let Some(status) = child.try_wait()? {
                anyhow::bail!("Control host exited during startup ({status})");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // A slow startup is not an instruction retry. The host receives no
        // provider turn until the caller explicitly submits one.
        anyhow::bail!("Local host startup timed out; reconnect without resending any instruction")
    }

    pub fn snapshot(&self) -> Result<ControlSnapshot> {
        self.call(Request::Snapshot, Duration::from_millis(500))
    }

    pub fn start_codex(
        &self,
        project: impl Into<PathBuf>,
        prompt: impl Into<String>,
        operation_id: impl Into<String>,
    ) -> Result<OperationReceipt> {
        self.mutate(Request::Start {
            project: project.into(),
            prompt: prompt.into(),
            operation_id: operation_id.into(),
        })
    }

    pub fn send_codex(
        &self,
        thread_id: impl Into<String>,
        prompt: impl Into<String>,
        expected_turn_id: Option<String>,
        operation_id: impl Into<String>,
    ) -> Result<OperationReceipt> {
        self.mutate(Request::Send {
            thread_id: thread_id.into(),
            prompt: prompt.into(),
            expected_turn_id,
            operation_id: operation_id.into(),
        })
    }

    pub fn interrupt_codex(
        &self,
        thread_id: impl Into<String>,
        turn_id: impl Into<String>,
        operation_id: impl Into<String>,
    ) -> Result<OperationReceipt> {
        self.mutate(Request::Interrupt {
            thread_id: thread_id.into(),
            turn_id: turn_id.into(),
            operation_id: operation_id.into(),
        })
    }

    pub fn respond(
        &self,
        request_id: impl Into<String>,
        response: Value,
        operation_id: impl Into<String>,
    ) -> Result<OperationReceipt> {
        self.mutate(Request::Reply {
            request_id: request_id.into(),
            response,
            operation_id: operation_id.into(),
        })
    }

    pub fn reconnect_codex(
        &self,
        thread_id: impl Into<String>,
        operation_id: impl Into<String>,
    ) -> Result<OperationReceipt> {
        self.mutate(Request::Reconnect {
            thread_id: thread_id.into(),
            operation_id: operation_id.into(),
        })
    }

    fn mutate(&self, request: Request) -> Result<OperationReceipt> {
        self.call(request, Duration::from_millis(self.config.rpc_timeout_ms * 3 + 2_000))
            .context("Control result is uncertain if the request was written. Inspect its operation ID; never resend automatically")
    }

    fn call<T: DeserializeOwned>(&self, request: Request, timeout: Duration) -> Result<T> {
        let endpoint: Endpoint = serde_json::from_slice(&security::read_private(
            &self.config.state_dir.join("endpoint.json"),
        )?)?;
        anyhow::ensure!(
            endpoint.address.ip().is_loopback(),
            "Refusing non-local control endpoint"
        );
        anyhow::ensure!(
            endpoint.codex_home == self.config.codex_home,
            "Control source home mismatch"
        );
        let mut stream = TcpStream::connect_timeout(&endpoint.address, Duration::from_millis(300))?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;
        serde_json::to_writer(
            &mut stream,
            &Envelope {
                token: endpoint.token,
                request,
            },
        )?;
        stream.write_all(b"\n")?;
        let line = security::read_line_limit(&mut BufReader::new(stream), 16 * 1024 * 1024)?
            .context("Local host closed before acknowledgement")?;
        let response: Response = serde_json::from_str(&line)?;
        if let Some(error) = response.error {
            anyhow::bail!("{error}");
        }
        Ok(serde_json::from_value(
            response.value.context("No host response")?,
        )?)
    }
}
