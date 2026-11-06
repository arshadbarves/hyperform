//! Deno: `deno.json`/`deno.jsonc` tasks with check names run through
//! `deno task`, and `deno test` unless a usable `test` task exists.

use serde_json::Value;
use z_engine_protocol::CheckKind;

use crate::discovery::collector::{Collector, brief};
use crate::discovery::jsonc;
use crate::discovery::rel;
use crate::discovery::scripts::{CANDIDATES, unusable};
use crate::discovery::walk::Found;

pub(crate) fn is_manifest(name: &str) -> bool {
    matches!(name, "deno.json" | "deno.jsonc")
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let mut dirs: Vec<&str> = found.iter().map(|f| f.dir.as_str()).collect();
    dirs.dedup();
    for dir in dirs {
        // Deno prefers deno.json when both exist.
        let Some(file) = ["deno.json", "deno.jsonc"]
            .into_iter()
            .find(|name| found.iter().any(|f| f.dir == dir && f.name == *name))
        else {
            continue;
        };
        let manifest = rel::join(dir, file);
        let Some(text) = c.read(&manifest) else {
            continue;
        };
        let config = match serde_json::from_str::<Value>(&jsonc::strip(&text)) {
            Ok(Value::Object(config)) => config,
            Ok(_) => {
                c.note(format!("{manifest}: not a JSON object"));
                continue;
            }
            Err(error) => {
                c.note(format!("{manifest}: invalid JSON ({})", brief(&error)));
                continue;
            }
        };
        c.add_root(dir, "deno", &manifest);
        let tasks = config.get("tasks").and_then(Value::as_object);
        let mut test_task = false;
        for (name, kind) in CANDIDATES {
            let Some(task) = tasks.and_then(|t| t.get(*name)) else {
                continue;
            };
            let body = task
                .as_str()
                .or_else(|| task.get("command").and_then(Value::as_str));
            let Some(body) = body else {
                continue;
            };
            if let Some(reason) = unusable(name, *kind, body) {
                c.note(format!(
                    "{manifest}: task `{name}` is not a check: {reason}"
                ));
                continue;
            }
            c.check(
                dir,
                &manifest,
                &format!("deno:{name}"),
                *kind,
                format!("deno task {name}"),
            );
            test_task |= *name == "test";
        }
        if !test_task {
            c.check(dir, &manifest, "deno:test", CheckKind::Test, "deno test");
        }
    }
}
