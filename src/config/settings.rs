use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub search_mode: bool,
    pub search_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCallStats {
    pub calls: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub local_version: String,
    pub remote_version: String,
    pub has_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    // Basic configuration
    pub password: String,
    pub web_password: String,
    pub gemini_api_keys: Vec<String>,
    pub port: Option<u16>,

    // Streaming configuration
    pub fake_streaming: bool,
    pub fake_streaming_interval: f64,
    pub fake_streaming_chunk_size: i32,
    pub fake_streaming_delay_per_chunk: f64,

    // Storage configuration
    pub storage_dir: String,
    pub enable_storage: bool,

    // Concurrency configuration
    pub concurrent_requests: usize,
    pub increase_concurrent_on_failure: usize,
    pub max_concurrent_requests: usize,

    // Cache configuration
    pub cache_expiry_time: u64,
    pub max_cache_entries: usize,
    pub calculate_cache_entries: usize,
    pub precise_cache: bool,

    // Vertex AI configuration
    pub enable_vertex: bool,
    pub google_credentials_json: String,
    pub enable_vertex_express: bool,
    pub vertex_express_api_key: String,

    // Search configuration
    pub search: SearchConfig,

    // Security configuration
    pub random_string: bool,
    pub random_string_length: usize,
    pub max_empty_responses: usize,
    pub show_api_error_message: bool,

    // Rate limiting
    pub max_retry_num: usize,
    pub max_requests_per_minute: u32,
    pub max_requests_per_day_per_ip: u32,
    pub api_key_daily_limit: u32,

    // Model filtering
    pub blocked_models: HashSet<String>,
    pub whitelist_models: HashSet<String>,
    pub whitelist_user_agent: HashSet<String>,

    // Other configuration
    pub public_mode: bool,
    pub dashboard_url: String,
    pub allowed_origins: Vec<String>,

    // Network configuration
    pub nonstream_keepalive_enabled: bool,
    pub nonstream_keepalive_interval: f64,

    // Runtime information
    pub base_dir: PathBuf,
    pub invalid_api_keys: Vec<String>,
    pub version: VersionInfo,
    pub api_call_stats: ApiCallStats,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            password: "123".to_string(),
            web_password: "123".to_string(),
            gemini_api_keys: Vec::new(),
            port: Some(7860),

            fake_streaming: true,
            fake_streaming_interval: 1.0,
            fake_streaming_chunk_size: 10,
            fake_streaming_delay_per_chunk: 0.1,

            storage_dir: "/rujimi/settings/".to_string(),
            enable_storage: false,

            concurrent_requests: 1,
            increase_concurrent_on_failure: 0,
            max_concurrent_requests: 3,

            cache_expiry_time: 21600, // 6 hours
            max_cache_entries: 500,
            calculate_cache_entries: 6,
            precise_cache: false,

            enable_vertex: false,
            google_credentials_json: String::new(),
            enable_vertex_express: false,
            vertex_express_api_key: String::new(),

            search: SearchConfig {
                search_mode: false,
                search_prompt: "（使用搜索工具联网搜索，需要在content中结合搜索内容）".to_string(),
            },

            random_string: true,
            random_string_length: 5,
            max_empty_responses: 5,
            show_api_error_message: true,

            max_retry_num: 15,
            max_requests_per_minute: 30,
            max_requests_per_day_per_ip: 600,
            api_key_daily_limit: 100,

            blocked_models: HashSet::new(),
            whitelist_models: HashSet::new(),
            whitelist_user_agent: HashSet::new(),

            public_mode: false,
            dashboard_url: String::new(),
            allowed_origins: Vec::new(),

            nonstream_keepalive_enabled: true,
            nonstream_keepalive_interval: 5.0,

            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            invalid_api_keys: Vec::new(),
            version: VersionInfo {
                local_version: "0.0.0".to_string(),
                remote_version: "0.0.0".to_string(),
                has_update: false,
            },
            api_call_stats: ApiCallStats {
                calls: Vec::new(),
            },
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        let mut settings = Self::default();

        // Load from environment variables
        settings.password = env::var("PASSWORD").unwrap_or_else(|_| "123".to_string()).trim_matches('"').to_string();
        settings.web_password = env::var("WEB_PASSWORD").unwrap_or_else(|_| settings.password.clone()).trim_matches('"').to_string();

        // Parse API keys
        if let Ok(keys_str) = env::var("GEMINI_API_KEYS") {
            settings.gemini_api_keys = keys_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if let Ok(port_str) = env::var("PORT") {
            settings.port = Some(port_str.parse().unwrap_or(7860));
        }

        // Boolean configurations
        settings.fake_streaming = parse_bool(&env::var("FAKE_STREAMING").unwrap_or_else(|_| "true".to_string()));
        settings.enable_storage = parse_bool(&env::var("ENABLE_STORAGE").unwrap_or_else(|_| "false".to_string()));
        settings.enable_vertex = parse_bool(&env::var("ENABLE_VERTEX").unwrap_or_else(|_| "false".to_string()));
        settings.enable_vertex_express = parse_bool(&env::var("ENABLE_VERTEX_EXPRESS").unwrap_or_else(|_| "false".to_string()));
        settings.search.search_mode = parse_bool(&env::var("SEARCH_MODE").unwrap_or_else(|_| "false".to_string()));
        settings.random_string = parse_bool(&env::var("RANDOM_STRING").unwrap_or_else(|_| "true".to_string()));
        settings.show_api_error_message = parse_bool(&env::var("SHOW_API_ERROR_MESSAGE").unwrap_or_else(|_| "true".to_string()));
        settings.precise_cache = parse_bool(&env::var("PRECISE_CACHE").unwrap_or_else(|_| "false".to_string()));
        settings.public_mode = parse_bool(&env::var("PUBLIC_MODE").unwrap_or_else(|_| "false".to_string()));
        settings.nonstream_keepalive_enabled = parse_bool(&env::var("NONSTREAM_KEEPALIVE_ENABLED").unwrap_or_else(|_| "true".to_string()));

        // String configurations
        settings.storage_dir = env::var("STORAGE_DIR").unwrap_or_else(|_| "/rujimi/settings/".to_string());
        settings.google_credentials_json = env::var("GOOGLE_CREDENTIALS_JSON").unwrap_or_default();
        settings.vertex_express_api_key = env::var("VERTEX_EXPRESS_API_KEY").unwrap_or_default();
        settings.search.search_prompt = env::var("SEARCH_PROMPT")
            .unwrap_or_else(|_| "（使用搜索工具联网搜索，需要在content中结合搜索内容）".to_string())
            .trim_matches('"').to_string();
        settings.dashboard_url = env::var("DASHBOARD_URL").unwrap_or_default();

        // Numeric configurations
        settings.fake_streaming_interval = env::var("FAKE_STREAMING_INTERVAL")
            .unwrap_or_else(|_| "1".to_string()).parse().unwrap_or(1.0);
        settings.fake_streaming_chunk_size = env::var("FAKE_STREAMING_CHUNK_SIZE")
            .unwrap_or_else(|_| "10".to_string()).parse().unwrap_or(10);
        settings.fake_streaming_delay_per_chunk = env::var("FAKE_STREAMING_DELAY_PER_CHUNK")
            .unwrap_or_else(|_| "0.1".to_string()).parse().unwrap_or(0.1);
        settings.concurrent_requests = env::var("CONCURRENT_REQUESTS")
            .unwrap_or_else(|_| "1".to_string()).parse().unwrap_or(1);
        settings.increase_concurrent_on_failure = env::var("INCREASE_CONCURRENT_ON_FAILURE")
            .unwrap_or_else(|_| "0".to_string()).parse().unwrap_or(0);
        settings.max_concurrent_requests = env::var("MAX_CONCURRENT_REQUESTS")
            .unwrap_or_else(|_| "3".to_string()).parse().unwrap_or(3);
        settings.cache_expiry_time = env::var("CACHE_EXPIRY_TIME")
            .unwrap_or_else(|_| "21600".to_string()).parse().unwrap_or(21600);
        settings.max_cache_entries = env::var("MAX_CACHE_ENTRIES")
            .unwrap_or_else(|_| "500".to_string()).parse().unwrap_or(500);
        settings.calculate_cache_entries = env::var("CALCULATE_CACHE_ENTRIES")
            .unwrap_or_else(|_| "6".to_string()).parse().unwrap_or(6);
        settings.random_string_length = env::var("RANDOM_STRING_LENGTH")
            .unwrap_or_else(|_| "5".to_string()).parse().unwrap_or(5);
        settings.max_empty_responses = env::var("MAX_EMPTY_RESPONSES")
            .unwrap_or_else(|_| "5".to_string()).parse().unwrap_or(5);
        settings.max_retry_num = env::var("MAX_RETRY_NUM")
            .unwrap_or_else(|_| "15".to_string()).parse().unwrap_or(15);
        settings.max_requests_per_minute = env::var("MAX_REQUESTS_PER_MINUTE")
            .unwrap_or_else(|_| "30".to_string()).parse().unwrap_or(30);
        settings.max_requests_per_day_per_ip = env::var("MAX_REQUESTS_PER_DAY_PER_IP")
            .unwrap_or_else(|_| "600".to_string()).parse().unwrap_or(600);
        settings.api_key_daily_limit = env::var("API_KEY_DAILY_LIMIT")
            .unwrap_or_else(|_| "100".to_string()).parse().unwrap_or(100);
        settings.nonstream_keepalive_interval = env::var("NONSTREAM_KEEPALIVE_INTERVAL")
            .unwrap_or_else(|_| "5.0".to_string()).parse().unwrap_or(5.0);

        // List/Set configurations
        settings.blocked_models = parse_comma_separated_set(&env::var("BLOCKED_MODELS").unwrap_or_default());
        settings.whitelist_models = parse_comma_separated_set(&env::var("WHITELIST_MODELS").unwrap_or_default());
        settings.whitelist_user_agent = parse_comma_separated_set_lowercase(&env::var("WHITELIST_USER_AGENT").unwrap_or_default());
        settings.allowed_origins = parse_comma_separated(&env::var("ALLOWED_ORIGINS").unwrap_or_default());
        settings.invalid_api_keys = parse_comma_separated(&env::var("INVALID_API_KEYS").unwrap_or_default());

        // Set base directory
        if let Ok(current_dir) = env::current_dir() {
            if let Some(parent) = current_dir.parent() {
                if let Some(parent) = parent.parent() {
                    settings.base_dir = parent.to_path_buf();
                }
            }
        }

        Ok(settings)
    }

    pub fn get_valid_api_keys(&self) -> Vec<String> {
        self.gemini_api_keys
            .iter()
            .filter(|key| !self.invalid_api_keys.contains(key))
            .cloned()
            .collect()
    }

    pub fn update_invalid_keys(&mut self, invalid_keys: Vec<String>) {
        self.invalid_api_keys = invalid_keys;
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "true" | "1" | "yes")
}

fn parse_comma_separated(value: &str) -> Vec<String> {
    if value.trim().is_empty() {
        Vec::new()
    } else {
        value
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

fn parse_comma_separated_set(value: &str) -> HashSet<String> {
    if value.trim().is_empty() {
        HashSet::new()
    } else {
        value
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

fn parse_comma_separated_set_lowercase(value: &str) -> HashSet<String> {
    if value.trim().is_empty() {
        HashSet::new()
    } else {
        value
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }
}