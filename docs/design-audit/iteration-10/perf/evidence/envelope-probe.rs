use super::*;
use serde_json::json;
use std::io::Write;
use std::time::Instant;

fn pending_snapshot(source: &ClaudeSource) -> Value {
    source.pending_tools.stats()
}

fn rss_kib() -> Option<u64> {
    let output = std::process::Command::new("ps")
        .args(["-p", &std::process::id().to_string(), "-o", "rss="])
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

fn validate_audit_output(output: &Path, audit: &Path) -> io::Result<()> {
    let relative = output.strip_prefix(audit).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "output must be inside target/audit",
        )
    })?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        || output.exists()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output must be a new target/audit descendant without traversal",
        ));
    }
    let mut checked = audit.to_path_buf();
    for component in relative.components() {
        checked.push(component);
        match fs::symlink_metadata(&checked) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "output path contains a symlink or file",
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[test]
fn resource_output_guard_rejects_overwrite_traversal_and_outside_locations() {
    let f = Fixture::new();
    let audit = f.root.join("audit");
    fs::create_dir(&audit).unwrap();
    assert!(validate_audit_output(&audit.join("new/nested"), &audit).is_ok());
    assert!(validate_audit_output(&audit, &audit).is_err());
    assert!(validate_audit_output(&audit.join("../escape"), &audit).is_err());
    assert!(validate_audit_output(&f.root.join("outside"), &audit).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&f.root, audit.join("redirect")).unwrap();
        assert!(validate_audit_output(&audit.join("redirect/output"), &audit).is_err());
    }
}

struct Fixture {
    root: PathBuf,
    transcript: PathBuf,
    source: ClaudeSource,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "theywork-correlation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("workspace/.git")).unwrap();
        let home = root.join("claude");
        let transcript = home.join("projects/fixture/root.jsonl");
        fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        File::create(&transcript).unwrap();
        let source = ClaudeSource::new(&home);
        Self {
            root,
            transcript,
            source,
        }
    }

    fn append(&self, kind: &str, at: i64, content: Value) {
        let value = json!({"type":kind,"timestamp":at,"sessionId":"root","cwd":self.root.join("workspace"),"message":{"content":[content]}});
        self.append_record(value);
    }

    fn append_record(&self, value: Value) {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&self.transcript)
            .unwrap();
        writeln!(file, "{value}").unwrap();
    }

    fn tool(&self, id: &str, at: i64, name: &str, input: Value) {
        self.append(
            "assistant",
            at,
            json!({"type":"tool_use","id":id,"name":name,"input":input}),
        );
    }

    fn result(&self, id: &str, at: i64) {
        self.append(
            "user",
            at,
            json!({"type":"tool_result","tool_use_id":id,"content":"Exit code: 0\nrecorded"}),
        );
    }

    fn poll(&mut self, now: i64) -> Vec<Event> {
        let before = fs::read(&self.transcript).ok();
        let events = self.source.poll(now).unwrap();
        assert_eq!(
            fs::read(&self.transcript).ok(),
            before,
            "collector changed source bytes"
        );
        events
    }
}

#[test]
fn background_launch_ack_keeps_correlation_for_an_explicit_late_completion() {
    let mut f = Fixture::new();
    f.tool(
        "background",
        1000,
        "Agent",
        json!({"prompt":"Check recorded facts","run_in_background":true}),
    );
    f.poll(1001);
    for (at, status, text) in [
        (1002, "running", "Agent launched"),
        (1004, "completed", "Actual child output"),
    ] {
        f.append_record(json!({"type":"user","timestamp":at,"sessionId":"root","cwd":f.root.join("workspace"),"toolUseResult":{"agentId":"child","status":status},"message":{"content":[{"type":"tool_result","tool_use_id":"background","content":text}]}}));
        let events = f.poll(at + 1);
        let results: Vec<_> = events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::Collaboration(result) if result.kind == CollaborationKind::Result => {
                    Some(result)
                }
                _ => None,
            })
            .collect();
        if status == "running" {
            assert!(results.is_empty());
            assert_eq!(pending_snapshot(&f.source)["entries"], 1);
        } else {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].text.as_deref(), Some("Actual child output"));
            assert!(results[0].recipient.is_some());
            assert_eq!(pending_snapshot(&f.source)["entries"], 0);
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn correlation_coverage(events: &[Event]) -> theywork_core::ToolCorrelationCoverage {
    events
        .iter()
        .rev()
        .find_map(|event| match &event.kind {
            EventKind::Coverage(coverage) => coverage.tool_correlation.clone(),
            _ => None,
        })
        .expect("collector correlation coverage")
}

#[test]
fn retired_late_results_do_not_invent_activity_and_healthy_polls_preserve_losses() {
    let mut f = Fixture::new();
    for seq in 0..257 {
        // Timestamps deliberately reverse; retirement must follow observation order.
        f.tool(
            &format!("call-{seq}"),
            1000 - seq,
            "Bash",
            json!({"command":format!("command-{seq}")}),
        );
    }
    let mut world = theywork_core::World::new();
    let events = f.poll(2000);
    assert_eq!(correlation_coverage(&events).evicted, 1);
    for event in events {
        world.apply(event);
    }
    f.result("call-0", 2001);
    f.result("call-256", 2002);
    let events = f.poll(2003);
    let outcomes: Vec<_> = events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Did(beat) if beat.outcome.is_some() => Some(beat),
            _ => None,
        })
        .collect();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].activity.detail(), Some("command-256"));
    assert_eq!(outcomes[0].outcome, Some(Outcome::Exited(0)));
    for event in events {
        world.apply(event);
    }
    let healthy = f.poll(2004);
    assert_eq!(correlation_coverage(&healthy).evicted, 1);
    for event in healthy {
        world.apply(event);
    }
    let worker = world
        .offices()
        .flat_map(|o| o.workers.iter())
        .next()
        .unwrap();
    assert!(worker.coverage.available);
    assert_eq!(
        worker.coverage.tool_correlation.as_ref().unwrap().evicted,
        1
    );
}

#[test]
fn oversized_id_keeps_activity_without_storing_or_misclassifying_results() {
    let mut f = Fixture::new();
    let id = "x".repeat(pending::CURSOR_BYTES + 1);
    f.tool(
        &id,
        1000,
        "Bash",
        json!({"command":"oversized-id-observation"}),
    );
    f.tool("read", 1001, "Read", json!({"file_path":"src/read.rs"}));
    let events = f.poll(1002);
    assert!(events.iter().any(|e|matches!(&e.kind,EventKind::Acted(Activity::Typing{detail}) if detail=="oversized-id-observation")));
    assert_eq!(correlation_coverage(&events).oversized, 1);
    assert_eq!(pending_snapshot(&f.source)["entries"], 0);
    f.result(&id, 1003);
    f.result("read", 1004);
    let events = f.poll(1005);
    assert!(!events.iter().any(|e| matches!(e.kind, EventKind::Did(_))));
    assert_eq!(
        correlation_coverage(&events).oversized,
        1,
        "ordinary untracked results are not counted as retirement"
    );
}

#[test]
fn duplicate_and_conflicting_input_beyond_caption_limit_never_change_recipient_facts() {
    let mut f = Fixture::new();
    let prefix = "x".repeat(140);
    f.tool(
        "same",
        1000,
        "Bash",
        json!({"command":format!("{prefix} first")}),
    );
    f.tool(
        "same",
        1001,
        "Bash",
        json!({"command":format!("{prefix} first")}),
    );
    let events = f.poll(1002);
    assert_eq!(pending_snapshot(&f.source)["entries"], 1);
    assert_eq!(correlation_coverage(&events).ambiguous, 0);
    f.tool(
        "same",
        1003,
        "Bash",
        json!({"command":format!("{prefix} different")}),
    );
    let events = f.poll(1004);
    assert_eq!(correlation_coverage(&events).ambiguous, 1);
    f.result("same", 1005);
    let events = f.poll(1006);
    assert!(!events.iter().any(|e| matches!(e.kind, EventKind::Did(_))));
    assert_eq!(pending_snapshot(&f.source)["entries"], 0);
    assert_eq!(pending_snapshot(&f.source)["lookup_capacity"], 0);
}

#[test]
fn rewrite_and_disconnection_release_pending_state_without_erasing_history_loss() {
    let mut f = Fixture::new();
    let mut world = theywork_core::World::new();
    f.tool("unresolved", 1000, "Bash", json!({"command":"old-command"}));
    for event in f.poll(1001) {
        world.apply(event);
    }
    fs::write(&f.transcript, "").unwrap();
    let reset = f.poll(1002);
    assert_eq!(correlation_coverage(&reset).reset_discarded, 1);
    for event in reset {
        world.apply(event);
    }
    assert_eq!(pending_snapshot(&f.source)["entries"], 0);
    assert_eq!(pending_snapshot(&f.source)["lookup_capacity"], 0);
    f.tool("current", 1003, "Bash", json!({"command":"new-command"}));
    f.result("unresolved", 1004);
    f.result("current", 1005);
    let after = f.poll(1006);
    assert_eq!(
        after
            .iter()
            .filter(|e| matches!(
                e.kind,
                EventKind::Did(Beat {
                    outcome: Some(_),
                    ..
                })
            ))
            .count(),
        1
    );
    for event in after {
        world.apply(event);
    }
    let worker = world
        .offices()
        .flat_map(|o| o.workers.iter())
        .next()
        .unwrap();
    assert!(worker
        .coverage
        .tool_correlation
        .as_ref()
        .unwrap()
        .has_loss());
    assert!(
        worker
            .coverage
            .tool_correlation
            .as_ref()
            .unwrap()
            .prior_loss
    );
    f.tool(
        "pending-remove",
        1007,
        "Bash",
        json!({"command":"never-run"}),
    );
    for event in f.poll(1008) {
        world.apply(event);
    }
    fs::remove_dir_all(f.root.join("claude/projects")).unwrap();
    for event in f.source.poll(1009).unwrap() {
        world.apply(event);
    }
    assert_eq!(pending_snapshot(&f.source)["entries"], 0);
    assert_eq!(pending_snapshot(&f.source)["cursors"], 0);
    let worker = world
        .offices()
        .flat_map(|o| o.workers.iter())
        .next()
        .unwrap();
    assert!(!worker.coverage.available);
    assert!(worker
        .coverage
        .tool_correlation
        .as_ref()
        .unwrap()
        .has_loss());
}

#[test]
fn replacement_worker_does_not_inherit_the_previous_workers_loss_or_correlation() {
    let mut f = Fixture::new();
    let mut world = theywork_core::World::new();
    f.tool("old", 1000, "Bash", json!({"command":"old-worker-command"}));
    let first = f.poll(1001);
    let old = first
        .iter()
        .find_map(|e| matches!(e.kind, EventKind::Identity { .. }).then_some(e.worker.clone()))
        .unwrap();
    for event in first {
        world.apply(event);
    }
    fs::write(&f.transcript, "").unwrap();
    for event in f.poll(1002) {
        world.apply(event);
    }
    f.append_record(json!({"type":"user","timestamp":1003,"sessionId":"different-worker","cwd":f.root.join("workspace"),"message":{"content":[{"type":"tool_result","tool_use_id":"old","content":"Exit code: 0"}]}}));
    let after = f.poll(1004);
    assert!(!after.iter().any(|e| matches!(e.kind, EventKind::Did(_))));
    let new = after
        .iter()
        .find_map(|e| matches!(e.kind, EventKind::Identity { .. }).then_some(e.worker.clone()))
        .unwrap();
    assert_ne!(old, new);
    for event in after {
        world.apply(event);
    }
    assert!(world
        .worker(&old)
        .unwrap()
        .coverage
        .tool_correlation
        .as_ref()
        .unwrap()
        .has_loss());
    assert!(!world
        .worker(&new)
        .unwrap()
        .coverage
        .tool_correlation
        .as_ref()
        .unwrap()
        .has_loss());
}

#[test]
#[ignore = "bounded resource replay; use an isolated target/audit output directory"]
fn correlation_resource_replay() {
    let root =
        PathBuf::from(std::env::var_os("THEYWORK_PERF_OUTPUT").expect("THEYWORK_PERF_OUTPUT"));
    let mode = std::env::var("THEYWORK_PERF_MODE").expect("THEYWORK_PERF_MODE");
    assert!(mode == "paired" || mode == "unpaired");
    let overlap: usize = std::env::var("THEYWORK_PERF_OVERLAP")
        .expect("THEYWORK_PERF_OVERLAP").parse().unwrap();
    assert!([1, 8, 32, 128].contains(&overlap));
    let batches = 2560 / overlap;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let audit = workspace.join("target/audit");
    fs::create_dir_all(&audit).unwrap();
    let audit = fs::canonicalize(audit).unwrap();
    let root = if root.is_absolute() {
        root
    } else {
        std::env::current_dir().unwrap().join(root)
    };
    validate_audit_output(&root, &audit).expect("isolated resource output");
    fs::create_dir_all(root.join("workspace/.git")).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let home = root.join("claude");
    let paths: Vec<_> = (0..50)
        .map(|worker| {
            let path = home.join(format!("projects/fixture/task-{worker:02}.jsonl"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            File::create(&path).unwrap();
            path
        })
        .collect();
    let mut source = ClaudeSource::new(&home);
    let mut samples = Vec::new();
    let started = Instant::now();
    let mut activities = 0;
    let mut outcomes = 0;
    for batch in 0..batches {
        for phase in 0..if mode == "paired" { 2 } else { 1 } {
            for (worker, path) in paths.iter().enumerate() {
                let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
                for call in 0..overlap {
                    let seq = batch * overlap + call;
                    let id = format!("call-{worker:02}-{seq:06}");
                    let content = if phase == 0 {
                        if seq % 2 == 0 {
                            json!({"type":"tool_use","id":id,"name":"Bash","input":{"command":"echo synthetic-observation-only"}})
                        } else {
                            json!({"type":"tool_use","id":id,"name":"Edit","input":{"file_path":"src/fixture_界.rs","old_string":"old\nline","new_string":"new\nline\nextra"}})
                        }
                    } else {
                        json!({"type":"tool_result","tool_use_id":id,"content":"Exit code: 0\nSynthetic recorded outcome"})
                    };
                    let value = json!({"type":if phase==0 {"assistant"} else {"user"},"timestamp":1_000_000+batch*2+phase,"sessionId":format!("fixture-{worker:02}"),"cwd":root.join("workspace"),"message":{"content":[content]}});
                    writeln!(file, "{value}").unwrap();
                }
            }
            let poll_started = Instant::now();
            let events = source.poll(1_000_000 + (batch * 2 + phase) as i64).unwrap();
            let poll_ms = poll_started.elapsed().as_secs_f64() * 1000.;
            activities += events
                .iter()
                .filter(|e| matches!(e.kind, EventKind::Acted(_)))
                .count();
            outcomes += events
                .iter()
                .filter(|e| {
                    matches!(
                        e.kind,
                        EventKind::Did(Beat {
                            outcome: Some(_),
                            ..
                        })
                    )
                })
                .count();
            let mut sample = pending_snapshot(&source);
            assert!(sample["entries"].as_u64().unwrap() <= pending::SOURCE_ENTRIES as u64);
            assert!(sample["owned_string_bytes"].as_u64().unwrap() <= pending::SOURCE_BYTES as u64);
            assert_eq!(sample["entries"], sample["order_entries"]);
            if mode == "paired" {
                assert_eq!(sample["entries"], if phase == 0 { 50 * overlap } else { 0 });
                assert_eq!(sample["evicted"], 0);
                assert_eq!(sample["oversized"], 0);
                assert_eq!(sample["ambiguous"], 0);
            }
            sample["batch"] = json!(batch);
            sample["phase"] = json!(phase);
            sample["poll_ms"] = json!(poll_ms);
            sample["rss_kib"] = json!(rss_kib());
            samples.push(sample);
        }
    }
    assert_eq!(activities, 128_000);
    assert_eq!(outcomes, if mode == "paired" { 128_000 } else { 0 });
    let result = json!({"mode":mode,"calls":128_000,"workers":50,"overlap_per_worker":overlap,"activities":activities,"outcomes":outcomes,"elapsed_seconds":started.elapsed().as_secs_f64(),"samples":samples,"scope":"Direct ClaudeSource fixture replay in a Rust test process; sampled RSS is separate from owned-string accounting; no live provider, World or terminal."});
    fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("audit result: {}", root.join("result.json").display());
}
