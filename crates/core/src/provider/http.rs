//! Shared request pipeline for providers that wrap every answer in a status envelope (TorBox, AllDebrid).

use super::Timing;
use crate::error::{Error, Result};
use crate::models::{Msg, Provider};
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use reqwest::{Method, RequestBuilder};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use url::Url;

pub type Build<'a> = &'a (dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync);

pub struct ProviderHttp {
    http: reqwest::Client,
    token: String,
    provider: Provider,
    base: Url,
    spacing: Duration,
    retry_base: Duration,
    next: Mutex<Instant>,
}

impl ProviderHttp {
    pub fn new(http: reqwest::Client, token: &str, provider: Provider, default_base: &str, base: Option<&str>, timing: Timing) -> Self {
        let mut text = base.unwrap_or(default_base).to_string();
        if !text.ends_with('/') {
            text.push('/');
        }
        ProviderHttp {
            http,
            token: token.to_string(),
            provider,
            base: Url::parse(&text).expect("provider base URL"),
            spacing: timing.gate_other,
            retry_base: timing.retry_base,
            next: Mutex::new(Instant::now()),
        }
    }

    pub fn redact(&self, value: &str) -> String {
        if self.token.is_empty() {
            return value.to_string();
        }
        let encoded = percent_encoding::utf8_percent_encode(&self.token, percent_encoding::NON_ALPHANUMERIC).to_string();
        value.replace(&self.token, "[redacted]").replace(&encoded, "[redacted]")
    }

    async fn pace(&self) {
        let mut next = self.next.lock().await;
        let now = Instant::now();
        if *next > now {
            tokio::time::sleep(*next - now).await;
        }
        *next = Instant::now() + self.spacing;
    }

    /// Returns the envelope's `data`. Only safe reads and explicit rate limits are replayed, never an ambiguous
    /// upload or delete.
    pub async fn send(&self, method: Method, path: &str, build: Build<'_>, safe_read: bool) -> Result<Value> {
        let url = self.base.join(path).map_err(|e| Error::invalid(e.to_string()))?;
        let mut attempt = 0u32;
        loop {
            self.pace().await;
            let request = build(self.http.request(method.clone(), url.clone()).bearer_auth(&self.token));
            let response = request.send().await.map_err(|_| {
                Error::Network(format!("{}: không kết nối được máy chủ.", self.provider.label()))
            })?;
            let status = response.status();
            if attempt < 3 && (status.as_u16() == 429 || (safe_read && status.as_u16() >= 500)) {
                let wait = retry_after(&response).unwrap_or(self.retry_base * (attempt + 1));
                // Shared cool-down: every other request to this service waits too.
                {
                    let mut next = self.next.lock().await;
                    let until = Instant::now() + if wait.is_zero() { Duration::from_millis(5) } else { wait };
                    if until > *next {
                        *next = until;
                    }
                }
                attempt += 1;
                continue;
            }
            let body = response.text().await.map_err(|_| Error::Network(self.provider.label().to_string()))?;
            let root: Value = serde_json::from_str(&body).map_err(|_| Error::Api {
                provider: self.provider,
                status: status.as_u16(),
                code: "INVALID_RESPONSE".into(),
                message: "Máy chủ trả về dữ liệu không hợp lệ.".into(),
                error_code: None,
            })?;
            let ok = if self.provider == Provider::TorBox { flag(&root, "success") } else { str_of(&root, "status") == "success" };
            if !status.is_success() || !ok {
                let error = root.get("error").cloned().unwrap_or(Value::Null);
                let (code, message) = if error.is_object() {
                    (str_of(&error, "code"), str_of(&error, "message"))
                } else {
                    (str_of(&root, "error"), str_of(&root, "detail"))
                };
                let mapped = match code.as_str() {
                    "AUTH_BAD_APIKEY" | "AUTH_MISSING_APIKEY" | "BAD_TOKEN" | "NO_AUTH" => 401,
                    "MAGNET_INVALID_ID" | "ITEM_NOT_FOUND" => 404,
                    _ => status.as_u16(),
                };
                let (code, message) = (self.redact(&code), self.redact(&message));
                return Err(Error::Api {
                    provider: self.provider,
                    status: mapped,
                    message: format!("{}: {} · {}", self.provider.label(), code, message),
                    code,
                    error_code: None,
                });
            }
            return Ok(root.get("data").cloned().unwrap_or(Value::Null));
        }
    }
}

pub fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    response.headers().get(reqwest::header::RETRY_AFTER)?.to_str().ok()?.trim().parse::<u64>().ok().map(Duration::from_secs)
}

pub fn str_of(value: &Value, name: &str) -> String {
    match value.get(name) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(true)) => "True".into(),
        Some(Value::Bool(false)) => "False".into(),
        Some(other) => other.to_string(),
    }
}

pub fn num(value: &Value, name: &str) -> i64 {
    str_of(value, name).parse().unwrap_or(0)
}

pub fn decimal(value: &Value, name: &str) -> f64 {
    str_of(value, name).parse().unwrap_or(0.0)
}

pub fn flag(value: &Value, name: &str) -> bool {
    matches!(value.get(name), Some(Value::Bool(true)))
}

pub fn prop<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.get(name).unwrap_or(&Value::Null)
}

pub fn items(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(list) => list.iter().collect(),
        Value::Object(_) => vec![value],
        _ => vec![],
    }
}

pub fn date(value: &Value, name: &str) -> Option<DateTime<FixedOffset>> {
    let text = str_of(value, name);
    if let Ok(seconds) = text.parse::<i64>() {
        if seconds > 0 && seconds < 253_402_300_800 {
            return Utc.timestamp_opt(seconds, 0).single().map(|d| d.fixed_offset());
        }
    }
    DateTime::parse_from_rfc3339(&text).ok()
}

#[allow(dead_code)]
pub fn localized(key: &str) -> Error {
    Error::Operation(Msg::plain(key))
}
