pub mod api_key;
pub mod auth;
pub mod browser;
pub mod cache;
pub mod error_handling;
pub mod logging;
pub mod maintenance;
pub mod rate_limiting;
pub mod request;
pub mod response;
pub mod stats;
pub mod version;

// Re-export commonly used items from logging
// Note: Custom logging functions exist for compatibility but are not used since rujimi uses tracing
#[allow(dead_code)]
pub use logging::{log, vertex_log, LogEntry, VertexLogEntry, LogManager, VertexLogManager};

// Re-export commonly used items from request
// Note: Request management exists but is currently handled differently in rujimi
#[allow(dead_code)]
pub use request::{ActiveRequest, ActiveRequestsManager, RequestStatistics, GLOBAL_REQUEST_MANAGER};

// Re-export commonly used items from response
// Note: Response utilities exist but are handled by services in current architecture
#[allow(dead_code)]
pub use response::{
    openai_from_text, gemini_from_text, openai_from_gemini,
    create_completion_chunk, create_final_chunk, create_models_response
};

// Re-export commonly used items from maintenance
// Note: Maintenance functions exist but are not currently integrated into rujimi's startup flow
#[allow(dead_code)]
pub use maintenance::{
    MaintenanceScheduler, setup_global_exception_handler,
    handle_exception_with_context, api_call_stats_clean, emergency_cleanup
};

// Re-export from other modules for convenience
pub use api_key::{ApiKeyManager, ApiKeyStats};
pub use auth::{AuthState, AuthResult, AuthScope};
pub use cache::{ResponseCacheManager, CacheEntry, CacheStats};
pub use error_handling::{translate_error, ErrorContext};

// Note: Rate limiting and version checking exist but are not currently active
#[allow(dead_code)]
pub use rate_limiting::{RateLimiter, RateLimitError, RateLimitInfo};

pub use stats::{ApiStatsManager, ApiCallRecord, ApiStats, ModelStats};

#[allow(dead_code)]
pub use version::{VersionInfo, check_for_updates};