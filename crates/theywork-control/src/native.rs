//! Native console handoff, using an executable and separate arguments only.
//! The host must suspend raw mode and its alternate screen before `run` and
//! restore them afterwards, including when process launch fails.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use theywork_core::{Agent, ThreadIdentity};

#[derive(Debug, Clone)]
pub struct NativeProvider {
    pub provider: Agent,
    pub program: PathBuf,
    pub home: PathBuf,
    pub version: String,
    pub background_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}

impl NativeCommand {
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.cwd)
            .envs(&self.env)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        command
    }

    pub fn run(&self) -> Result<ExitStatus> {
        // Signal dispositions are process-wide. Only one inherited console
        // handoff may be active, including across callers on different threads.
        static HANDOFF: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _handoff = HANDOFF
            .lock()
            .map_err(|_| anyhow::anyhow!("Console handoff lock failed"))?;
        let _signals = ConsoleSignals::install()?;
        let mut command = self.command();
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: signal is async-signal-safe; no allocation in pre_exec.
            unsafe {
                command.pre_exec(|| {
                    libc::signal(libc::SIGINT, libc::SIG_DFL);
                    libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                    Ok(())
                });
            }
        }
        Ok(command.status()?)
    }
}

#[cfg(unix)]
struct ConsoleSignals {
    interrupt: libc::sigaction,
    quit: libc::sigaction,
}

#[cfg(unix)]
impl ConsoleSignals {
    fn install() -> Result<Self> {
        // SAFETY: sigaction structures are initialized, remain live during the
        // calls, and previous dispositions are restored by Drop even on error.
        unsafe {
            let mut ignored: libc::sigaction = std::mem::zeroed();
            let mut interrupt: libc::sigaction = std::mem::zeroed();
            let mut quit: libc::sigaction = std::mem::zeroed();
            ignored.sa_sigaction = libc::SIG_IGN;
            libc::sigemptyset(&mut ignored.sa_mask);
            if libc::sigaction(libc::SIGINT, &ignored, &mut interrupt) != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            if libc::sigaction(libc::SIGQUIT, &ignored, &mut quit) != 0 {
                let error = std::io::Error::last_os_error();
                libc::sigaction(libc::SIGINT, &interrupt, std::ptr::null_mut());
                return Err(error.into());
            }
            Ok(Self { interrupt, quit })
        }
    }
}

#[cfg(unix)]
impl Drop for ConsoleSignals {
    fn drop(&mut self) {
        // SAFETY: these are complete dispositions returned by sigaction.
        unsafe {
            libc::sigaction(libc::SIGINT, &self.interrupt, std::ptr::null_mut());
            libc::sigaction(libc::SIGQUIT, &self.quit, std::ptr::null_mut());
        }
    }
}

#[cfg(windows)]
struct ConsoleSignals;

#[cfg(windows)]
unsafe extern "system" fn handoff_handler(signal: u32) -> i32 {
    i32::from(matches!(signal, 0 | 1)) // CTRL_C_EVENT and CTRL_BREAK_EVENT
}

#[cfg(windows)]
impl ConsoleSignals {
    fn install() -> Result<Self> {
        // A handler registration is not inherited by the child; unlike the
        // process ignore flag, it leaves native Ctrl-C handling intact.
        if unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(handoff_handler), 1)
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for ConsoleSignals {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(handoff_handler), 0);
        }
    }
}

#[cfg(not(any(unix, windows)))]
struct ConsoleSignals;
#[cfg(not(any(unix, windows)))]
impl ConsoleSignals {
    fn install() -> Result<Self> {
        anyhow::bail!("Native console signals unsupported")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeSession {
    pub cwd: PathBuf,
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "waitingFor")]
    pub waiting_for: Option<String>,
}

impl NativeProvider {
    /// Read-only version/help probes, with a deadline. This never logs in,
    /// starts a model turn, or calls `resume`/`respawn`.
    pub fn detect(provider: Agent, home: impl AsRef<Path>) -> Result<Self> {
        let name = match provider {
            Agent::Codex => "codex",
            Agent::Claude => "claude",
        };
        let program = find_executable(name)
            .with_context(|| format!("Install the official {name} CLI to use its console"))?;
        Self::from_program(provider, home, program)
    }

    pub fn from_program(
        provider: Agent,
        home: impl AsRef<Path>,
        program: impl Into<PathBuf>,
    ) -> Result<Self> {
        let home = std::fs::canonicalize(home)?;
        let mut value = Self {
            provider,
            home,
            program: program.into(),
            version: String::new(),
            background_supported: false,
        };
        value.version = value.probe(&["--version"])?.trim().to_owned();
        anyhow::ensure!(
            !value.version.is_empty(),
            "Provider did not report a version"
        );
        if provider == Agent::Claude {
            let help = value.probe(&["--help"])?;
            value.background_supported = help
                .split_whitespace()
                .any(|word| word.trim_end_matches(',') == "--bg")
                && std::env::var_os("CLAUDE_CODE_DISABLE_AGENT_VIEW").is_none();
        }
        Ok(value)
    }

    pub fn login(&self, cwd: impl AsRef<Path>) -> Result<NativeCommand> {
        self.spec(
            cwd,
            match self.provider {
                Agent::Codex => vec!["login".into()],
                Agent::Claude => vec!["auth".into(), "login".into()],
            },
        )
    }

    pub fn new_conversation(&self, cwd: impl AsRef<Path>, prompt: &str) -> Result<NativeCommand> {
        anyhow::ensure!(
            !prompt.trim().is_empty() && prompt.len() <= 64 * 1024,
            "Instruction must contain 1–65536 bytes"
        );
        let args = if self.provider == Agent::Claude && self.background_supported {
            vec!["--bg".into(), "--".into(), prompt.into()]
        } else {
            // -- separates a user prompt beginning with '-' from CLI flags.
            vec!["--".into(), prompt.into()]
        };
        self.spec(cwd, args)
    }

    pub fn claude_sessions(&self) -> Result<Vec<ClaudeSession>> {
        anyhow::ensure!(
            self.provider == Agent::Claude && self.background_supported,
            "This installed CLI does not expose background agent view"
        );
        let json = self.probe(&["agents", "--json"])?;
        let value: Value = serde_json::from_str(&json)
            .context("This Claude version does not return the documented agent roster")?;
        anyhow::ensure!(value.is_array(), "Claude agent roster must be an array");
        Ok(serde_json::from_value(value)?)
    }

    /// Fresh same-source roster verification is mandatory. A saved transcript
    /// or a stopped job is never an attach capability and is never resumed.
    pub fn attach(&self, identity: &ThreadIdentity) -> Result<NativeCommand> {
        anyhow::ensure!(
            self.provider == Agent::Claude && identity.provider == Agent::Claude,
            "Native attach is available only for verified Claude background sessions"
        );
        anyhow::ensure!(
            identity.source.0 == self.home.to_string_lossy(),
            "Native session belongs to a different source home"
        );
        anyhow::ensure!(
            identity.session_id.is_none(),
            "Open the parent conversation to control this subagent"
        );
        let sessions = self.claude_sessions()?;
        let session = sessions.into_iter().find(|session| session.session_id.as_deref() == Some(&identity.native_id)
            && session.kind == "background" && session.pid.is_some_and(|pid| pid > 0)
            && !matches!(session.state.as_deref(), Some("failed" | "stopped")))
            .context("No live background session is bound to this conversation; open the original Claude console")?;
        let id = session.id.context("Background session has no attach ID")?;
        anyhow::ensure!(
            !id.is_empty()
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
            "Invalid native attach ID"
        );
        self.spec(session.cwd, vec!["attach".into(), id])
    }

    fn spec(&self, cwd: impl AsRef<Path>, args: Vec<String>) -> Result<NativeCommand> {
        let cwd = std::fs::canonicalize(cwd)?;
        anyhow::ensure!(cwd.is_dir(), "Console working directory is missing");
        let env = BTreeMap::from([(
            match self.provider {
                Agent::Codex => "CODEX_HOME",
                Agent::Claude => "CLAUDE_CONFIG_DIR",
            }
            .into(),
            self.home.to_string_lossy().into_owned(),
        )]);
        Ok(NativeCommand {
            program: self.program.clone(),
            args,
            cwd,
            env,
        })
    }

    fn probe(&self, args: &[&str]) -> Result<String> {
        let mut command = Command::new(&self.program);
        command.args(args).env(
            match self.provider {
                Agent::Codex => "CODEX_HOME",
                Agent::Claude => "CLAUDE_CONFIG_DIR",
            },
            &self.home,
        );
        capture(command, Duration::from_secs(5))
    }
}

pub fn find_executable(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&paths) {
        #[cfg(windows)]
        let candidates = [directory.join(format!("{name}.exe"))];
        #[cfg(not(windows))]
        let candidates = [directory.join(name)];
        for path in candidates {
            if !path.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if path.metadata().ok()?.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }
            return std::fs::canonicalize(path).ok();
        }
    }
    None
}

fn capture(mut command: Command, timeout: Duration) -> Result<String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let output = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = output
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Provider probe timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    anyhow::ensure!(
        status.success(),
        "Provider command is unavailable or failed ({status})"
    );
    let bytes = receiver
        .recv_timeout(timeout.saturating_sub(start.elapsed()))
        .context("Provider output timed out")??;
    anyhow::ensure!(bytes.len() <= 1024 * 1024, "Provider output exceeds limit");
    Ok(String::from_utf8(bytes)?)
}
