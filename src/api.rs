// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! API powered text stickers.
//!
//! Requests run on a worker thread and report back through a channel, so the
//! event loop and the renderer never block on the network. The number of
//! in-flight requests is bounded by the scene itself (one per API sticker).

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Instant;

use serde_json::Value;

use crate::scene::{ApiSpec, Node, Scene};

pub struct ApiManager {
    sender: Sender<ApiResult>,
    receiver: Receiver<ApiResult>,
    last_fetch: HashMap<u64, Instant>,
    in_flight: HashSet<u64>,
}

pub struct ApiResult {
    pub uid: u64,
    pub text: String,
}

impl Default for ApiManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiManager {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            sender,
            receiver,
            last_fetch: HashMap::new(),
            in_flight: HashSet::new(),
        }
    }

    pub fn reset(&mut self) {
        self.last_fetch.clear();
        self.in_flight.clear();
        // Drain stale results from the previous scene generation.
        while self.receiver.try_recv().is_ok() {}
    }

    /// Start due requests and apply finished ones. Returns `true` when text
    /// changed.
    pub fn tick(&mut self, scene: &mut Scene, now: Instant) -> bool {
        let mut changed = false;
        loop {
            match self.receiver.try_recv() {
                Ok(result) => {
                    self.in_flight.remove(&result.uid);
                    self.last_fetch.insert(result.uid, now);
                    if let Some(node) = scene.find_by_uid_mut(result.uid) {
                        changed |= node.set_text(result.text);
                        if let Some(content) = node.text_content_mut() {
                            content.loading = false;
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        let mut requests = Vec::new();
        collect_requests(scene, &mut requests);
        for node in requests {
            let Some(content) = node.text_content() else {
                continue;
            };
            let Some(spec) = &content.api else {
                continue;
            };
            if self.in_flight.contains(&node.uid) {
                continue;
            }
            let due = self
                .last_fetch
                .get(&node.uid)
                .is_none_or(|last| now.duration_since(*last) >= spec.update_interval);
            if !due {
                continue;
            }
            self.in_flight.insert(node.uid);
            let uid = node.uid;
            let spec = spec.clone();
            let sender = self.sender.clone();
            std::thread::Builder::new()
                .name(format!("widdgyy-api-{uid}"))
                .spawn(move || {
                    let text = fetch(&spec).unwrap_or_else(|error| {
                        log::warn!("API request failed: {error:#}");
                        "API error".to_string()
                    });
                    let _ = sender.send(ApiResult { uid, text });
                })
                .ok();
        }
        changed
    }

    pub fn has_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }
}

fn collect_requests<'a>(scene: &'a mut Scene, out: &mut Vec<&'a Node>) {
    for root in &scene.roots {
        collect_node(root, out);
    }
}

fn collect_node<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    if node
        .text_content()
        .is_some_and(|content| content.api.is_some())
    {
        out.push(node);
    }
    for child in &node.children {
        collect_node(child, out);
    }
}

pub fn fetch(spec: &ApiSpec) -> anyhow::Result<String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(spec.timeout))
        .build();
    let agent: ureq::Agent = config.into();

    let mut last_error = None;
    for _ in 0..=spec.retries {
        let result = perform(&agent, spec);
        match result {
            Ok(data) => {
                return format_response(&data, spec);
            }
            Err(error) => {
                last_error = Some(error);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("request failed")))
}

fn perform(agent: &ureq::Agent, spec: &ApiSpec) -> anyhow::Result<Value> {
    let mut response = match spec.method.as_str() {
        "POST" => add_headers(agent.post(&spec.url), &spec.headers).send("")?,
        "PUT" => add_headers(agent.put(&spec.url), &spec.headers).send("")?,
        "PATCH" => add_headers(agent.patch(&spec.url), &spec.headers).send("")?,
        "DELETE" => add_headers(agent.delete(&spec.url), &spec.headers).call()?,
        "HEAD" => add_headers(agent.head(&spec.url), &spec.headers).call()?,
        _ => add_headers(agent.get(&spec.url), &spec.headers).call()?,
    };
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {status}");
    }
    let body = response.body_mut().read_to_string()?;
    Ok(serde_json::from_str(&body)?)
}

fn add_headers<B>(
    mut request: ureq::RequestBuilder<B>,
    headers: &[(String, String)],
) -> ureq::RequestBuilder<B> {
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    request
}

fn format_response(data: &Value, spec: &ApiSpec) -> anyhow::Result<String> {
    let mut text = spec.template.clone();
    for (name, expression) in &spec.fields {
        let value = resolve_field(data, expression)?;
        text = text.replace(&format!("{{{name}}}"), &value);
    }
    if spec.fields.is_empty() {
        text = data.to_string();
    }
    Ok(text)
}

pub fn resolve_field(data: &Value, expression: &str) -> anyhow::Result<String> {
    let mut parts = expression.split('|');
    let path = parts.next().unwrap_or("").trim();
    let mut value = extract_path(data, path);
    for filter in parts.map(str::trim).filter(|filter| !filter.is_empty()) {
        value = apply_filter(&value, filter)?;
    }
    Ok(match value {
        Value::String(value) => value,
        other => other.to_string(),
    })
}

fn extract_path(data: &Value, path: &str) -> Value {
    let mut current = data;
    let mut token = String::new();
    let mut chars = path.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            '[' => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
                let mut index = String::new();
                for inner in chars.by_ref() {
                    if inner == ']' {
                        break;
                    }
                    index.push(inner);
                }
                tokens.push(format!("[{index}]"));
            }
            other => token.push(other),
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }

    for token in tokens {
        if let Some(index) = token
            .strip_prefix('[')
            .and_then(|token| token.strip_suffix(']'))
            .and_then(|index| index.parse::<usize>().ok())
        {
            current = current.get(index).unwrap_or(&Value::Null);
        } else {
            current = current.get(&token).unwrap_or(&Value::Null);
        }
    }
    current.clone()
}

fn apply_filter(value: &Value, filter: &str) -> anyhow::Result<Value> {
    match filter {
        "weekday_short" => {
            let text = value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("weekday_short expects a string"))?;
            let normalized = text.replace('Z', "+00:00");
            let weekday = if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&normalized) {
                date.format("%a").to_string()
            } else if let Ok(date) = chrono::NaiveDate::parse_from_str(&normalized, "%Y-%m-%d") {
                date.format("%a").to_string()
            } else {
                anyhow::bail!("weekday_short expects an RFC3339 date");
            };
            Ok(Value::String(weekday))
        }
        other => anyhow::bail!("unsupported field filter: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_paths() {
        let data = serde_json::json!({"daily": {"time": ["2026-01-01", "2026-01-02"]}});
        assert_eq!(resolve_field(&data, "daily.time[1]").unwrap(), "2026-01-02");
        assert_eq!(
            resolve_field(&data, "daily.time[0]|weekday_short").unwrap(),
            "Thu"
        );
    }
}
