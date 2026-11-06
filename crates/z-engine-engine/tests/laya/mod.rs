//! A local stand-in for laya-serve's `POST /v1/systemone`: every question
//! is answered by `rule(question name, state)` with confidence 0.99. Rules
//! return an option key; yes/no questions go out as `a` (yes) and `b` (no).
//! `silent` accepts requests and never answers them.

#![allow(dead_code)]

pub mod workout;

use serde_json::{Map, Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use z_engine_config::FEATURES;

pub type Rule = fn(&str, &Value) -> &'static str;

pub const YES: &str = "a";
pub const NO: &str = "b";

/// The fake answers at once; a loaded machine can still starve a short
/// timeout into an abstain (today's behavior), so tests that expect an
/// answer allow this much. The use deadline (twice this, plus slack)
/// stays inside the event recorder's 10 s wait.
pub const ANSWER_TIMEOUT_MS: u64 = 4000;

/// Starts the server; returns its endpoint.
pub async fn laya(rule: Rule) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(answer(stream, rule));
        }
    });
    format!("http://{addr}")
}

/// `[decisions]` pointing at `endpoint`, plus `features` switched on.
pub fn settings(base: &str, endpoint: &str, features: &[&str]) -> String {
    let mut toml = format!("{base}\n[experimental]\n");
    for feature in features {
        toml.push_str(&format!("{feature} = \"on\"\n"));
    }
    toml.push_str(&format!(
        "\n[decisions]\nendpoint = \"{endpoint}\"\ntimeout_ms = {ANSWER_TIMEOUT_MS}\n"
    ));
    toml
}

/// `[decisions]` pointing at `endpoint`, with every registered feature in
/// `mode` (`off`, `shadow` or `on`).
pub fn every_feature(base: &str, endpoint: &str, mode: &str, timeout_ms: u64) -> String {
    let mut toml = format!("{base}\n[experimental]\n");
    for spec in FEATURES {
        toml.push_str(&format!("{} = \"{mode}\"\n", spec.id));
    }
    toml.push_str(&format!(
        "\n[decisions]\nendpoint = \"{endpoint}\"\ntimeout_ms = {timeout_ms}\n"
    ));
    toml
}

/// Accepts connections and holds them open without a reply, so every
/// question runs into the timeout.
pub async fn silent() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream);
        }
    });
    format!("http://{addr}")
}

async fn answer(mut stream: TcpStream, rule: Rule) -> std::io::Result<()> {
    let mut seen = Vec::new();
    let mut chunk = [0u8; 8192];
    let (start, length) = loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        seen.extend_from_slice(&chunk[..read]);
        if let Some(at) = seen.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&seen[..at]).to_lowercase();
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            break (at + 4, length);
        }
    };
    while seen.len() < start + length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        seen.extend_from_slice(&chunk[..read]);
    }
    let request: Value = serde_json::from_slice(&seen[start..start + length])?;
    let names = request["questions"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let answers: Map<String, Value> = names
        .keys()
        .map(|name| {
            let choice = rule(name, &request["state"]);
            (
                name.clone(),
                json!({ "choice": choice, "answer_confidence": 0.99 }),
            )
        })
        .collect();
    let body = json!({ "answers": answers }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}
