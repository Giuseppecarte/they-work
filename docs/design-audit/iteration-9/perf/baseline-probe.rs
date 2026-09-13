// Reproduction probe for the unbounded collector at 77a9b785.
// The retained run used these workload/accounting semantics before formatting.
use super::*;
use serde_json::json;
use std::io::Write;
use std::time::Instant;

fn pending_snapshot(source: &ClaudeSource) -> Value {
    let entries: usize = source.files.values().map(|c| c.pending_tools.len()).sum();
    let capacities: usize = source.files.values().map(|c| c.pending_tools.capacity()).sum();
    let owned: usize = source.files.values().flat_map(|c| c.pending_tools.iter())
        .map(|(id, tool)| id.capacity() + activity_capacity(&tool.activity)).sum();
    json!({"entries":entries,"owned_string_bytes":owned,"lookup_capacity":capacities})
}
fn activity_capacity(activity: &Activity) -> usize {
    match activity {
        Activity::Typing{detail} | Activity::Reading{detail} | Activity::Editing{detail}
        | Activity::Searching{detail} | Activity::Talking{detail} | Activity::Waiting{detail}
        | Activity::Error{detail} => detail.capacity(),
        Activity::Thinking | Activity::Idle => 0,
    }
}
fn rss_kib() -> Option<u64> {
    let output = std::process::Command::new("ps")
        .args(["-p", &std::process::id().to_string(), "-o", "rss="])
        .output().ok()?;
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

#[test]
#[ignore = "bounded resource replay; use an isolated target/audit output directory"]
fn correlation_resource_replay() {
    let root =
        PathBuf::from(std::env::var_os("THEYWORK_PERF_OUTPUT").expect("THEYWORK_PERF_OUTPUT"));
    let mode = std::env::var("THEYWORK_PERF_MODE").expect("THEYWORK_PERF_MODE");
    assert!(mode == "paired" || mode == "unpaired");
    assert!(!root.exists(), "resource output must be new");
    fs::create_dir_all(root.join("workspace/.git")).unwrap();
    let root = fs::canonicalize(root).unwrap();
    assert!(root.components().any(|c| c.as_os_str() == "target"));
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
    for batch in 0..80 {
        for phase in 0..if mode == "paired" { 2 } else { 1 } {
            for (worker, path) in paths.iter().enumerate() {
                let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
                for call in 0..32 {
                    let seq = batch * 32 + call;
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
            sample["batch"] = json!(batch);
            sample["phase"] = json!(phase);
            sample["poll_ms"] = json!(poll_ms);
            sample["rss_kib"] = json!(rss_kib());
            samples.push(sample);
        }
    }
    assert_eq!(activities, 128_000);
    assert_eq!(outcomes, if mode == "paired" { 128_000 } else { 0 });
    let result = json!({"mode":mode,"calls":128_000,"workers":50,"overlap_per_worker":32,"activities":activities,"outcomes":outcomes,"elapsed_seconds":started.elapsed().as_secs_f64(),"samples":samples,"scope":"Direct ClaudeSource fixture replay in a Rust test process; sampled RSS is separate from owned-string accounting; no live provider, World or terminal."});
    fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("audit result: {}", root.join("result.json").display());
}
