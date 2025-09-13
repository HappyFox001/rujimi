use std::collections::{VecDeque, HashMap};
use std::sync::{Arc, RwLock, Mutex};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::fmt;

// Rust equivalent of Python utils/logging.py

const DEBUG: bool = false; // Can be configured from environment

// Log formats
const LOG_FORMAT_DEBUG: &str = "{timestamp} - {level} - [{key}]-{request_type}-[{model}]-{status_code}: {message} - {error_message}";
const LOG_FORMAT_NORMAL: &str = "[{timestamp}] [{level}] [{key}]-{request_type}-[{model}]-{status_code}: {message}";

// Vertex log formats
const VERTEX_LOG_FORMAT_DEBUG: &str = "{timestamp} - {level} - [{vertex_id}]-{operation}-[{status}]: {message} - {error_message}";
const VERTEX_LOG_FORMAT_NORMAL: &str = "[{timestamp}] [{level}] [{vertex_id}]-{operation}-[{status}]: {message}";

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub key: Option<String>,
    pub request_type: Option<String>,
    pub model: Option<String>,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
    pub extra: Option<HashMap<String, Value>>,
}

impl LogEntry {
    pub fn new(level: &str, message: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level: level.to_string(),
            message: message.to_string(),
            key: None,
            request_type: None,
            model: None,
            status_code: None,
            error_message: None,
            extra: None,
        }
    }

    pub fn with_key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    pub fn with_request_type(mut self, request_type: &str) -> Self {
        self.request_type = Some(request_type.to_string());
        self
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }

    pub fn with_error_message(mut self, error_message: &str) -> Self {
        self.error_message = Some(error_message.to_string());
        self
    }

    pub fn with_extra(mut self, extra: HashMap<String, Value>) -> Self {
        self.extra = Some(extra);
        self
    }
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let template = if DEBUG {
            LOG_FORMAT_DEBUG
        } else {
            LOG_FORMAT_NORMAL
        };

        let formatted = template
            .replace("{timestamp}", &self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
            .replace("{level}", &self.level)
            .replace("{key}", &self.key.as_deref().unwrap_or("-"))
            .replace("{request_type}", &self.request_type.as_deref().unwrap_or("-"))
            .replace("{model}", &self.model.as_deref().unwrap_or("-"))
            .replace("{status_code}", &self.status_code.map(|s| s.to_string()).as_deref().unwrap_or("-"))
            .replace("{message}", &self.message)
            .replace("{error_message}", &self.error_message.as_deref().unwrap_or(""));

        write!(f, "{}", formatted)
    }
}

#[derive(Debug, Clone)]
pub struct VertexLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub vertex_id: Option<String>,
    pub operation: Option<String>,
    pub status: Option<String>,
    pub error_message: Option<String>,
    pub extra: Option<HashMap<String, Value>>,
}

impl VertexLogEntry {
    pub fn new(level: &str, message: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level: level.to_string(),
            message: message.to_string(),
            vertex_id: None,
            operation: None,
            status: None,
            error_message: None,
            extra: None,
        }
    }

    pub fn with_vertex_id(mut self, vertex_id: &str) -> Self {
        self.vertex_id = Some(vertex_id.to_string());
        self
    }

    pub fn with_operation(mut self, operation: &str) -> Self {
        self.operation = Some(operation.to_string());
        self
    }

    pub fn with_status(mut self, status: &str) -> Self {
        self.status = Some(status.to_string());
        self
    }

    pub fn with_error_message(mut self, error_message: &str) -> Self {
        self.error_message = Some(error_message.to_string());
        self
    }
}

impl fmt::Display for VertexLogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let template = if DEBUG {
            VERTEX_LOG_FORMAT_DEBUG
        } else {
            VERTEX_LOG_FORMAT_NORMAL
        };

        let formatted = template
            .replace("{timestamp}", &self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
            .replace("{level}", &self.level)
            .replace("{vertex_id}", &self.vertex_id.as_deref().unwrap_or("-"))
            .replace("{operation}", &self.operation.as_deref().unwrap_or("-"))
            .replace("{status}", &self.status.as_deref().unwrap_or("-"))
            .replace("{message}", &self.message)
            .replace("{error_message}", &self.error_message.as_deref().unwrap_or(""));

        write!(f, "{}", formatted)
    }
}

/// Log cache for displaying recent logs on the web interface
pub struct LogManager {
    logs: Arc<RwLock<VecDeque<LogEntry>>>,
    max_logs: usize,
}

impl LogManager {
    pub fn new(max_logs: usize) -> Self {
        Self {
            logs: Arc::new(RwLock::new(VecDeque::with_capacity(max_logs))),
            max_logs,
        }
    }

    pub fn add_log(&self, entry: LogEntry) {
        let mut logs = self.logs.write().unwrap();

        // Print to stdout
        println!("{}", entry);

        // Add to cache
        logs.push_back(entry);

        // Keep only the last max_logs entries
        while logs.len() > self.max_logs {
            logs.pop_front();
        }
    }

    pub fn get_logs(&self) -> Vec<LogEntry> {
        let logs = self.logs.read().unwrap();
        logs.iter().cloned().collect()
    }

    pub fn get_recent_logs(&self, count: usize) -> Vec<LogEntry> {
        let logs = self.logs.read().unwrap();
        logs.iter()
            .rev()
            .take(count)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn get_logs_by_level(&self, level: &str) -> Vec<LogEntry> {
        let logs = self.logs.read().unwrap();
        logs.iter()
            .filter(|log| log.level.eq_ignore_ascii_case(level))
            .cloned()
            .collect()
    }

    pub fn get_logs_json(&self) -> Value {
        let logs = self.get_logs();
        let log_values: Vec<Value> = logs.into_iter().map(|log| {
            json!({
                "timestamp": log.timestamp.to_rfc3339(),
                "level": log.level,
                "message": log.message,
                "key": log.key,
                "request_type": log.request_type,
                "model": log.model,
                "status_code": log.status_code,
                "error_message": log.error_message,
                "extra": log.extra
            })
        }).collect();

        json!(log_values)
    }

    pub fn clear(&self) {
        let mut logs = self.logs.write().unwrap();
        logs.clear();
    }

    pub fn count(&self) -> usize {
        let logs = self.logs.read().unwrap();
        logs.len()
    }
}

/// Vertex-specific log manager
pub struct VertexLogManager {
    logs: Arc<RwLock<VecDeque<VertexLogEntry>>>,
    max_logs: usize,
}

impl VertexLogManager {
    pub fn new(max_logs: usize) -> Self {
        Self {
            logs: Arc::new(RwLock::new(VecDeque::with_capacity(max_logs))),
            max_logs,
        }
    }

    pub fn add_log(&self, entry: VertexLogEntry) {
        let mut logs = self.logs.write().unwrap();

        // Print to stdout
        println!("{}", entry);

        // Add to cache
        logs.push_back(entry);

        // Keep only the last max_logs entries
        while logs.len() > self.max_logs {
            logs.pop_front();
        }
    }

    pub fn get_logs(&self) -> Vec<VertexLogEntry> {
        let logs = self.logs.read().unwrap();
        logs.iter().cloned().collect()
    }

    pub fn get_recent_logs(&self, count: usize) -> Vec<VertexLogEntry> {
        let logs = self.logs.read().unwrap();
        logs.iter()
            .rev()
            .take(count)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn clear(&self) {
        let mut logs = self.logs.write().unwrap();
        logs.clear();
    }
}

// Global log manager instances
lazy_static::lazy_static! {
    pub static ref LOG_MANAGER: LogManager = LogManager::new(100);
    pub static ref VERTEX_LOG_MANAGER: VertexLogManager = VertexLogManager::new(100);
}

/// Format log message - equivalent to Python's format_log_message
pub fn format_log_message(
    level: &str,
    message: &str,
    extra: Option<HashMap<String, Value>>,
) -> LogEntry {
    let mut entry = LogEntry::new(level, message);

    if let Some(extra_data) = extra {
        if let Some(key) = extra_data.get("key").and_then(|v| v.as_str()) {
            entry = entry.with_key(key);
        }
        if let Some(request_type) = extra_data.get("request_type").and_then(|v| v.as_str()) {
            entry = entry.with_request_type(request_type);
        }
        if let Some(model) = extra_data.get("model").and_then(|v| v.as_str()) {
            entry = entry.with_model(model);
        }
        if let Some(status_code) = extra_data.get("status_code").and_then(|v| v.as_u64()) {
            entry = entry.with_status_code(status_code as u16);
        }
        if let Some(error_message) = extra_data.get("error_message").and_then(|v| v.as_str()) {
            entry = entry.with_error_message(error_message);
        }
        entry = entry.with_extra(extra_data);
    }

    entry
}

/// Format vertex log message - equivalent to Python's vertex_format_log_message
pub fn vertex_format_log_message(
    level: &str,
    message: &str,
    extra: Option<HashMap<String, Value>>,
) -> VertexLogEntry {
    let mut entry = VertexLogEntry::new(level, message);

    if let Some(extra_data) = extra {
        if let Some(vertex_id) = extra_data.get("vertex_id").and_then(|v| v.as_str()) {
            entry = entry.with_vertex_id(vertex_id);
        }
        if let Some(operation) = extra_data.get("operation").and_then(|v| v.as_str()) {
            entry = entry.with_operation(operation);
        }
        if let Some(status) = extra_data.get("status").and_then(|v| v.as_str()) {
            entry = entry.with_status(status);
        }
        if let Some(error_message) = extra_data.get("error_message").and_then(|v| v.as_str()) {
            entry = entry.with_error_message(error_message);
        }
    }

    entry
}

/// Main logging function - equivalent to Python's log()
pub fn log(level: &str, message: &str, extra: Option<HashMap<String, Value>>) {
    let entry = format_log_message(level, message, extra);
    LOG_MANAGER.add_log(entry);

    // Also log to standard Rust logging
    match level.to_lowercase().as_str() {
        "error" => log::error!("{}", message),
        "warn" | "warning" => log::warn!("{}", message),
        "info" => log::info!("{}", message),
        "debug" => log::debug!("{}", message),
        _ => log::info!("{}", message),
    }
}

/// Vertex logging function - equivalent to Python's vertex_log()
pub fn vertex_log(level: &str, message: &str, extra: Option<HashMap<String, Value>>) {
    let entry = vertex_format_log_message(level, message, extra);
    VERTEX_LOG_MANAGER.add_log(entry);

    // Also log to standard Rust logging with vertex prefix
    let vertex_message = format!("[VERTEX] {}", message);
    match level.to_lowercase().as_str() {
        "error" => log::error!("{}", vertex_message),
        "warn" | "warning" => log::warn!("{}", vertex_message),
        "info" => log::info!("{}", vertex_message),
        "debug" => log::debug!("{}", vertex_message),
        _ => log::info!("{}", vertex_message),
    }
}

/// Convenience logging macros
#[macro_export]
macro_rules! log_info {
    ($msg:expr) => {
        $crate::utils::logging::log("info", $msg, None);
    };
    ($msg:expr, $extra:expr) => {
        $crate::utils::logging::log("info", $msg, Some($extra));
    };
}

#[macro_export]
macro_rules! log_error {
    ($msg:expr) => {
        $crate::utils::logging::log("error", $msg, None);
    };
    ($msg:expr, $extra:expr) => {
        $crate::utils::logging::log("error", $msg, Some($extra));
    };
}

#[macro_export]
macro_rules! log_warn {
    ($msg:expr) => {
        $crate::utils::logging::log("warn", $msg, None);
    };
    ($msg:expr, $extra:expr) => {
        $crate::utils::logging::log("warn", $msg, Some($extra));
    };
}

#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => {
        $crate::utils::logging::log("debug", $msg, None);
    };
    ($msg:expr, $extra:expr) => {
        $crate::utils::logging::log("debug", $msg, Some($extra));
    };
}

#[macro_export]
macro_rules! vertex_log_info {
    ($msg:expr) => {
        $crate::utils::logging::vertex_log("info", $msg, None);
    };
    ($msg:expr, $extra:expr) => {
        $crate::utils::logging::vertex_log("info", $msg, Some($extra));
    };
}

#[macro_export]
macro_rules! vertex_log_error {
    ($msg:expr) => {
        $crate::utils::logging::vertex_log("error", $msg, None);
    };
    ($msg:expr, $extra:expr) => {
        $crate::utils::logging::vertex_log("error", $msg, Some($extra));
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new("info", "Test message")
            .with_key("test-key")
            .with_model("test-model")
            .with_status_code(200);

        assert_eq!(entry.level, "info");
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.key, Some("test-key".to_string()));
        assert_eq!(entry.model, Some("test-model".to_string()));
        assert_eq!(entry.status_code, Some(200));
    }

    #[test]
    fn test_vertex_log_entry_creation() {
        let entry = VertexLogEntry::new("error", "Vertex error")
            .with_vertex_id("vertex-1")
            .with_operation("chat")
            .with_status("failed");

        assert_eq!(entry.level, "error");
        assert_eq!(entry.message, "Vertex error");
        assert_eq!(entry.vertex_id, Some("vertex-1".to_string()));
        assert_eq!(entry.operation, Some("chat".to_string()));
        assert_eq!(entry.status, Some("failed".to_string()));
    }

    #[test]
    fn test_log_manager() {
        let manager = LogManager::new(5);

        // Add some logs
        for i in 0..10 {
            let entry = LogEntry::new("info", &format!("Test message {}", i));
            manager.add_log(entry);
        }

        // Should keep only the last 5
        let logs = manager.get_logs();
        assert_eq!(logs.len(), 5);

        // Check the content of the last log
        assert!(logs.last().unwrap().message.contains("Test message 9"));
    }

    #[test]
    fn test_format_log_message() {
        let mut extra = HashMap::new();
        extra.insert("key".to_string(), json!("test-key"));
        extra.insert("model".to_string(), json!("gpt-4"));
        extra.insert("status_code".to_string(), json!(200));

        let entry = format_log_message("info", "Test format", Some(extra));

        assert_eq!(entry.level, "info");
        assert_eq!(entry.message, "Test format");
        assert_eq!(entry.key, Some("test-key".to_string()));
        assert_eq!(entry.model, Some("gpt-4".to_string()));
        assert_eq!(entry.status_code, Some(200));
    }
}