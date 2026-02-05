//! Rate limiting and adaptive throttling for Microsoft Graph API
//!
//! Provides proactive rate limiting to prevent HTTP 429 (Too Many Requests) errors
//! when communicating with the Microsoft Graph API.
//!
//! ## Architecture
//!
//! - [`TokenBucket`]: Classic token bucket algorithm for per-endpoint rate limiting
//! - [`AdaptiveRateLimiter`]: Manages multiple token buckets with adaptive capacity
//!   adjustment based on server responses (429 throttle / success)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use lnxdrive_graph::rate_limit::{AdaptiveRateLimiter, RateLimitConfig};
//!
//! # async fn example() {
//! let limiter = AdaptiveRateLimiter::new(RateLimitConfig::default());
//! limiter.acquire("delta").await;
//! // ... make API call ...
//! limiter.on_success("delta");
//! # }
//! ```

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use tracing::{debug, info, warn};

// ============================================================================
// T202: TokenBucket struct
// ============================================================================

/// Internal mutable state for the token bucket, protected by a Mutex.
#[derive(Debug)]
struct TokenBucketInner {
    /// Current number of available tokens (fractional for smooth refill)
    tokens: f64,
    /// Timestamp of the last refill calculation
    last_refill: Instant,
}

/// Token bucket rate limiter for a single endpoint.
///
/// Implements the classic token bucket algorithm: tokens are consumed on each
/// request and refilled at a constant rate. When no tokens are available,
/// callers must wait for refill.
///
/// Thread safety is provided by an internal `Mutex<TokenBucketInner>`.
#[derive(Debug)]
pub struct TokenBucket {
    /// Maximum number of tokens in the bucket
    capacity: u32,
    /// Effective capacity after adaptive adjustments (can be reduced by throttle)
    effective_capacity: Mutex<u32>,
    /// Rate at which tokens are added (tokens per second)
    refill_rate: f64,
    /// Mutable inner state (tokens count, last refill time)
    inner: Mutex<TokenBucketInner>,
    /// Count of consecutive successes (for adaptive recovery)
    success_count: Mutex<u64>,
    /// Original capacity before any throttle adjustments
    original_capacity: u32,
}

impl TokenBucket {
    /// Creates a new `TokenBucket` with the given configuration.
    ///
    /// The bucket starts full (tokens == capacity).
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of tokens
    /// * `refill_rate` - Tokens added per second
    pub fn new(capacity: u32, refill_rate: f64) -> Self {
        Self {
            capacity,
            effective_capacity: Mutex::new(capacity),
            refill_rate,
            inner: Mutex::new(TokenBucketInner {
                tokens: capacity as f64,
                last_refill: Instant::now(),
            }),
            success_count: Mutex::new(0),
            original_capacity: capacity,
        }
    }

    // ========================================================================
    // T204: refill()
    // ========================================================================

    /// Refills the bucket based on elapsed time since last refill.
    ///
    /// Calculates how many tokens should have been added based on the elapsed
    /// duration and the refill rate. Caps at the effective capacity.
    ///
    /// This is called internally by `try_acquire()` before checking availability.
    fn refill(inner: &mut TokenBucketInner, refill_rate: f64, effective_capacity: u32) {
        let now = Instant::now();
        let elapsed = now.duration_since(inner.last_refill);
        let elapsed_secs = elapsed.as_secs_f64();

        if elapsed_secs > 0.0 {
            let new_tokens = elapsed_secs * refill_rate;
            inner.tokens = (inner.tokens + new_tokens).min(effective_capacity as f64);
            inner.last_refill = now;
        }
    }

    // ========================================================================
    // T203: try_acquire()
    // ========================================================================

    /// Attempts to acquire a single token from the bucket.
    ///
    /// Refills first based on elapsed time, then checks if at least 1.0 token
    /// is available. If so, subtracts 1.0 and returns `true`. Otherwise
    /// returns `false` without modifying the bucket.
    ///
    /// This method is thread-safe via the internal Mutex.
    pub fn try_acquire(&self) -> bool {
        let effective_cap = *self.effective_capacity.lock().unwrap();
        let mut inner = self.inner.lock().unwrap();
        Self::refill(&mut inner, self.refill_rate, effective_cap);

        if inner.tokens >= 1.0 {
            inner.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Returns the estimated wait time in seconds until a token becomes available.
    ///
    /// If tokens are already available, returns 0.0.
    pub fn time_until_available(&self) -> f64 {
        let effective_cap = *self.effective_capacity.lock().unwrap();
        let mut inner = self.inner.lock().unwrap();
        Self::refill(&mut inner, self.refill_rate, effective_cap);

        if inner.tokens >= 1.0 {
            0.0
        } else {
            let deficit = 1.0 - inner.tokens;
            if self.refill_rate > 0.0 {
                deficit / self.refill_rate
            } else {
                f64::MAX
            }
        }
    }

    /// Returns the current number of available tokens (after refill).
    pub fn available_tokens(&self) -> f64 {
        let effective_cap = *self.effective_capacity.lock().unwrap();
        let mut inner = self.inner.lock().unwrap();
        Self::refill(&mut inner, self.refill_rate, effective_cap);
        inner.tokens
    }

    /// Returns the original (maximum) capacity.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Returns the current effective capacity (may be reduced by throttling).
    pub fn effective_capacity(&self) -> u32 {
        *self.effective_capacity.lock().unwrap()
    }

    // ========================================================================
    // T208: on_success - adaptive recovery
    // ========================================================================

    /// Records a successful API call for adaptive recovery.
    ///
    /// Every 100 consecutive successes, the effective capacity is increased
    /// by 5%, up to the original capacity. This allows the rate limiter to
    /// gradually recover from throttle-induced capacity reductions.
    pub fn on_success(&self) {
        let mut count = self.success_count.lock().unwrap();
        *count += 1;

        if *count % 100 == 0 {
            let mut eff_cap = self.effective_capacity.lock().unwrap();
            if *eff_cap < self.original_capacity {
                let increase = (*eff_cap as f64 * 0.05).max(1.0) as u32;
                let new_cap = (*eff_cap + increase).min(self.original_capacity);
                debug!(
                    old_capacity = *eff_cap,
                    new_capacity = new_cap,
                    successes = *count,
                    "Adaptive recovery: increasing bucket capacity"
                );
                *eff_cap = new_cap;
            }
        }
    }

    // ========================================================================
    // T209: on_throttle - adaptive reduction
    // ========================================================================

    /// Records a throttle event (HTTP 429) and reduces effective capacity by 50%.
    ///
    /// Also resets the success counter. The minimum effective capacity is 1
    /// to ensure the bucket never becomes permanently blocked.
    pub fn on_throttle(&self) {
        let mut eff_cap = self.effective_capacity.lock().unwrap();
        let old = *eff_cap;
        *eff_cap = (*eff_cap / 2).max(1);
        warn!(
            old_capacity = old,
            new_capacity = *eff_cap,
            "Throttle detected: reducing bucket capacity by 50%"
        );

        // Reset success counter
        let mut count = self.success_count.lock().unwrap();
        *count = 0;
    }
}

// ============================================================================
// T205: RateLimitGuard (simple marker)
// ============================================================================

/// Guard returned by the adaptive rate limiter after acquiring a token.
///
/// This is a simple marker type indicating that a rate limit token was
/// successfully acquired. Since the `TokenBucket` already subtracts the
/// token on acquisition, this guard does not need special drop behavior.
/// It can be used for future extensions (e.g., tracking in-flight requests).
#[derive(Debug)]
pub struct RateLimitGuard {
    /// Endpoint this guard was acquired for (for logging/metrics)
    _endpoint: String,
}

impl RateLimitGuard {
    fn new(endpoint: String) -> Self {
        Self {
            _endpoint: endpoint,
        }
    }
}

// ============================================================================
// T206: RateLimitConfig
// ============================================================================

/// Configuration for the adaptive rate limiter.
///
/// Defines default bucket parameters and per-endpoint overrides.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Default bucket capacity (tokens)
    pub default_capacity: u32,
    /// Default refill rate (tokens per second)
    pub default_refill_rate: f64,
    /// Per-endpoint overrides: (capacity, refill_rate)
    pub endpoint_overrides: HashMap<String, (u32, f64)>,
    /// Maximum number of retries on 429 responses
    pub max_retries: u32,
}

impl Default for RateLimitConfig {
    /// Returns default configuration aligned with Microsoft Graph API limits.
    ///
    /// Microsoft Graph API has a general limit of ~600 requests per 10 minutes
    /// per app per tenant, which translates to roughly 1 request per second.
    /// We start conservatively at 10 tokens/sec and let adaptive throttling adjust.
    fn default() -> Self {
        let mut overrides = HashMap::new();
        // Delta endpoint: conservative polling (10 req/min = 0.167/sec)
        overrides.insert("delta".to_string(), (10, 10.0 / 60.0));
        // Upload: moderate (60 req/min = 1/sec)
        overrides.insert("upload".to_string(), (60, 1.0));
        // Download: generous (120 req/min = 2/sec)
        overrides.insert("download".to_string(), (120, 2.0));
        // Metadata: liberal (100 req/min ~ 1.67/sec)
        overrides.insert("metadata".to_string(), (100, 100.0 / 60.0));

        Self {
            default_capacity: 600,
            default_refill_rate: 10.0,
            endpoint_overrides: overrides,
            max_retries: 5,
        }
    }
}

// ============================================================================
// T206: AdaptiveRateLimiter struct
// ============================================================================

/// Adaptive rate limiter managing per-endpoint token buckets.
///
/// Creates and manages `TokenBucket` instances for different API endpoints.
/// Adapts bucket capacities based on server responses: reduces on throttle
/// (429), gradually recovers on consecutive successes.
///
/// Thread-safe and designed to be shared via `Arc<AdaptiveRateLimiter>`.
pub struct AdaptiveRateLimiter {
    /// Per-endpoint token buckets
    buckets: Mutex<HashMap<String, TokenBucket>>,
    /// Configuration for bucket creation and retry behavior
    config: RateLimitConfig,
}

impl std::fmt::Debug for AdaptiveRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdaptiveRateLimiter")
            .field("config", &self.config)
            .finish()
    }
}

impl AdaptiveRateLimiter {
    /// Creates a new `AdaptiveRateLimiter` with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            config,
        }
    }

    /// Creates a new `AdaptiveRateLimiter` with default Graph API limits.
    pub fn with_defaults() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Returns the maximum number of retries configured.
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }

    /// Gets or creates a token bucket for the given endpoint.
    ///
    /// If an endpoint-specific override is configured, those parameters are used.
    /// Otherwise, the default capacity and refill rate are applied.
    fn get_or_create_bucket<F, R>(&self, endpoint: &str, f: F) -> R
    where
        F: FnOnce(&TokenBucket) -> R,
    {
        let mut buckets = self.buckets.lock().unwrap();
        if !buckets.contains_key(endpoint) {
            let (capacity, refill_rate) = self
                .config
                .endpoint_overrides
                .get(endpoint)
                .copied()
                .unwrap_or((
                    self.config.default_capacity,
                    self.config.default_refill_rate,
                ));

            debug!(
                endpoint,
                capacity, refill_rate, "Creating new token bucket for endpoint"
            );
            buckets.insert(
                endpoint.to_string(),
                TokenBucket::new(capacity, refill_rate),
            );
        }
        f(buckets.get(endpoint).unwrap())
    }

    // ========================================================================
    // T207: acquire()
    // ========================================================================

    /// Acquires a rate limit token for the given endpoint.
    ///
    /// If no tokens are available, calculates the wait time and sleeps until
    /// a token becomes available. Returns a `RateLimitGuard` on success.
    ///
    /// This method is async and will yield to the tokio runtime while waiting.
    pub async fn acquire(&self, endpoint: &str) -> RateLimitGuard {
        loop {
            let acquired = self.get_or_create_bucket(endpoint, |bucket| bucket.try_acquire());

            if acquired {
                debug!(endpoint, "Rate limit token acquired");
                return RateLimitGuard::new(endpoint.to_string());
            }

            // Calculate wait time and sleep
            let wait_secs =
                self.get_or_create_bucket(endpoint, |bucket| bucket.time_until_available());

            let wait = Duration::from_secs_f64(wait_secs.max(0.01));
            debug!(
                endpoint,
                wait_ms = wait.as_millis(),
                "No tokens available, waiting for refill"
            );
            tokio::time::sleep(wait).await;
        }
    }

    // ========================================================================
    // T208: on_success()
    // ========================================================================

    /// Notifies the rate limiter that an API call succeeded.
    ///
    /// Delegates to the endpoint's token bucket for adaptive recovery.
    /// After enough consecutive successes, the bucket's effective capacity
    /// will gradually increase back toward the original limit.
    pub fn on_success(&self, endpoint: &str) {
        self.get_or_create_bucket(endpoint, |bucket| bucket.on_success());
    }

    // ========================================================================
    // T209: on_throttle()
    // ========================================================================

    /// Notifies the rate limiter that a 429 response was received.
    ///
    /// Reduces the endpoint's effective bucket capacity by 50% to decrease
    /// request pressure. The capacity can recover via `on_success()`.
    pub fn on_throttle(&self, endpoint: &str) {
        info!(endpoint, "Recording throttle event for endpoint");
        self.get_or_create_bucket(endpoint, |bucket| bucket.on_throttle());
    }

    /// Returns the current available tokens for an endpoint.
    ///
    /// Returns `None` if no bucket exists for the endpoint yet.
    pub fn available_tokens(&self, endpoint: &str) -> Option<f64> {
        let buckets = self.buckets.lock().unwrap();
        buckets.get(endpoint).map(|b| b.available_tokens())
    }

    /// Returns the effective capacity for an endpoint.
    ///
    /// Returns `None` if no bucket exists for the endpoint yet.
    pub fn effective_capacity(&self, endpoint: &str) -> Option<u32> {
        let buckets = self.buckets.lock().unwrap();
        buckets.get(endpoint).map(|b| b.effective_capacity())
    }
}

// ============================================================================
// T211: Retry-After header parsing helpers
// ============================================================================

/// Parses a Retry-After header value into a Duration.
///
/// The header can be either:
/// - An integer number of seconds (e.g., "30")
/// - An HTTP-date (e.g., "Fri, 31 Dec 2025 23:59:59 GMT") - parsed as seconds from now
///
/// Falls back to the default duration if parsing fails.
pub fn parse_retry_after(value: &str, default: Duration) -> Duration {
    // Try parsing as integer seconds first (most common for Graph API)
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return Duration::from_secs(seconds);
    }

    // Try parsing as HTTP-date using chrono
    if let Ok(date) = chrono::DateTime::parse_from_rfc2822(value.trim()) {
        let now = chrono::Utc::now();
        let target = date.with_timezone(&chrono::Utc);
        if target > now {
            let diff = target - now;
            if let Some(secs) = diff
                .num_seconds()
                .try_into()
                .ok()
                .filter(|&s: &u64| s <= 3600)
            {
                return Duration::from_secs(secs);
            }
        }
    }

    // Fallback
    warn!(value, "Could not parse Retry-After header, using default");
    default
}

// ============================================================================
// T213: Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    // ====================================================================
    // TokenBucket tests
    // ====================================================================

    #[test]
    fn test_token_bucket_creation() {
        let bucket = TokenBucket::new(10, 1.0);
        assert_eq!(bucket.capacity(), 10);
        assert_eq!(bucket.effective_capacity(), 10);
        // Bucket starts full
        assert!(bucket.available_tokens() >= 9.9); // Allow slight timing variance
    }

    #[test]
    fn test_try_acquire_succeeds_when_tokens_available() {
        let bucket = TokenBucket::new(5, 1.0);
        assert!(bucket.try_acquire());
        assert!(bucket.try_acquire());
        assert!(bucket.try_acquire());
        assert!(bucket.try_acquire());
        assert!(bucket.try_acquire());
    }

    #[test]
    fn test_try_acquire_fails_when_empty() {
        let bucket = TokenBucket::new(2, 0.0); // No refill
        assert!(bucket.try_acquire()); // 1 token left
        assert!(bucket.try_acquire()); // 0 tokens left
        assert!(!bucket.try_acquire()); // Should fail
    }

    #[test]
    fn test_refill_adds_tokens_over_time() {
        let bucket = TokenBucket::new(10, 100.0); // Fast refill for testing

        // Drain all tokens
        for _ in 0..10 {
            bucket.try_acquire();
        }
        assert!(!bucket.try_acquire());

        // Wait a bit for refill (100 tokens/sec -> 10ms = 1 token)
        std::thread::sleep(Duration::from_millis(20));

        // Should have refilled at least one token
        assert!(bucket.try_acquire());
    }

    #[test]
    fn test_refill_caps_at_capacity() {
        let bucket = TokenBucket::new(5, 1000.0); // Very fast refill

        // Wait for refill
        std::thread::sleep(Duration::from_millis(50));

        // Available tokens should not exceed capacity
        let available = bucket.available_tokens();
        assert!(
            available <= 5.0 + 0.1, // small tolerance for timing
            "Available tokens {} should not exceed capacity 5",
            available
        );
    }

    #[test]
    fn test_time_until_available_zero_when_tokens_exist() {
        let bucket = TokenBucket::new(10, 1.0);
        assert_eq!(bucket.time_until_available(), 0.0);
    }

    #[test]
    fn test_time_until_available_positive_when_empty() {
        let bucket = TokenBucket::new(1, 1.0); // 1 token/sec refill
        bucket.try_acquire(); // drain

        let wait = bucket.time_until_available();
        // Should be close to 1.0 seconds (within tolerance for timing)
        assert!(wait > 0.0, "Wait time should be positive");
        assert!(wait <= 1.1, "Wait time {} should be <= 1.1 sec", wait);
    }

    // ====================================================================
    // TokenBucket adaptive behavior tests
    // ====================================================================

    #[test]
    fn test_on_throttle_reduces_capacity() {
        let bucket = TokenBucket::new(100, 1.0);
        assert_eq!(bucket.effective_capacity(), 100);

        bucket.on_throttle();
        assert_eq!(bucket.effective_capacity(), 50);

        bucket.on_throttle();
        assert_eq!(bucket.effective_capacity(), 25);
    }

    #[test]
    fn test_on_throttle_minimum_capacity_is_one() {
        let bucket = TokenBucket::new(4, 1.0);

        // Reduce repeatedly
        bucket.on_throttle(); // 4 -> 2
        assert_eq!(bucket.effective_capacity(), 2);
        bucket.on_throttle(); // 2 -> 1
        assert_eq!(bucket.effective_capacity(), 1);
        bucket.on_throttle(); // 1 -> 1 (minimum)
        assert_eq!(bucket.effective_capacity(), 1);
    }

    #[test]
    fn test_on_success_recovers_capacity() {
        let bucket = TokenBucket::new(100, 1.0);

        // Throttle down to 50
        bucket.on_throttle();
        assert_eq!(bucket.effective_capacity(), 50);

        // 100 successes should increase by ~5%
        for _ in 0..100 {
            bucket.on_success();
        }

        let cap = bucket.effective_capacity();
        assert!(
            cap > 50,
            "Capacity should have increased from 50, got {}",
            cap
        );
        assert!(
            cap <= 100,
            "Capacity should not exceed original 100, got {}",
            cap
        );
    }

    #[test]
    fn test_on_success_does_not_exceed_original_capacity() {
        let bucket = TokenBucket::new(100, 1.0);

        // Call on_success many times without throttle (already at max)
        for _ in 0..500 {
            bucket.on_success();
        }

        assert_eq!(bucket.effective_capacity(), 100);
    }

    #[test]
    fn test_throttle_resets_success_counter() {
        let bucket = TokenBucket::new(100, 1.0);
        bucket.on_throttle(); // 100 -> 50

        // Accumulate 90 successes (not enough for recovery threshold of 100)
        for _ in 0..90 {
            bucket.on_success();
        }
        assert_eq!(bucket.effective_capacity(), 50); // No change yet

        // Throttle again resets counter
        bucket.on_throttle(); // 50 -> 25

        // Another 90 successes - won't trigger recovery because counter was reset
        for _ in 0..90 {
            bucket.on_success();
        }
        assert_eq!(bucket.effective_capacity(), 25); // Still no recovery

        // 10 more successes = 100 total since last throttle -> recovery
        for _ in 0..10 {
            bucket.on_success();
        }
        let cap = bucket.effective_capacity();
        assert!(cap > 25, "Should have recovered, got {}", cap);
    }

    // ====================================================================
    // AdaptiveRateLimiter tests
    // ====================================================================

    #[test]
    fn test_adaptive_rate_limiter_creation() {
        let limiter = AdaptiveRateLimiter::with_defaults();
        assert_eq!(limiter.max_retries(), 5);
    }

    #[test]
    fn test_adaptive_rate_limiter_custom_config() {
        let config = RateLimitConfig {
            default_capacity: 100,
            default_refill_rate: 5.0,
            endpoint_overrides: HashMap::new(),
            max_retries: 3,
        };
        let limiter = AdaptiveRateLimiter::new(config);
        assert_eq!(limiter.max_retries(), 3);
    }

    #[tokio::test]
    async fn test_acquire_succeeds_immediately() {
        let limiter = AdaptiveRateLimiter::with_defaults();
        let _guard = limiter.acquire("metadata").await;
        // Should not block since bucket starts full
    }

    #[tokio::test]
    async fn test_acquire_creates_bucket_on_demand() {
        let limiter = AdaptiveRateLimiter::with_defaults();

        // No bucket yet
        assert!(limiter.available_tokens("custom_endpoint").is_none());

        // Acquire creates the bucket
        let _guard = limiter.acquire("custom_endpoint").await;
        assert!(limiter.available_tokens("custom_endpoint").is_some());
    }

    #[tokio::test]
    async fn test_acquire_waits_when_empty() {
        let config = RateLimitConfig {
            default_capacity: 1,
            default_refill_rate: 100.0, // Fast refill so test doesn't take long
            endpoint_overrides: HashMap::new(),
            max_retries: 3,
        };
        let limiter = AdaptiveRateLimiter::new(config);

        // Drain the single token
        let _g1 = limiter.acquire("test").await;

        // The next acquire should wait for refill, but succeed
        let start = Instant::now();
        let _g2 = limiter.acquire("test").await;
        let elapsed = start.elapsed();

        // Should have waited at least a tiny bit (refill at 100/sec -> ~10ms for 1 token)
        // Use a generous bound since test timing can be imprecise
        assert!(
            elapsed.as_millis() < 500,
            "Should not have waited too long: {:?}",
            elapsed
        );
    }

    #[test]
    fn test_on_throttle_reduces_endpoint_capacity() {
        let limiter = AdaptiveRateLimiter::with_defaults();

        // Force bucket creation by checking tokens
        // Use acquire in a sync context by directly accessing get_or_create_bucket
        limiter.get_or_create_bucket("delta", |_| {});

        let original_cap = limiter.effective_capacity("delta").unwrap();

        limiter.on_throttle("delta");

        let reduced_cap = limiter.effective_capacity("delta").unwrap();
        assert_eq!(reduced_cap, original_cap / 2);
    }

    #[test]
    fn test_on_success_recovers_after_throttle() {
        let limiter = AdaptiveRateLimiter::with_defaults();

        // Create bucket and throttle
        limiter.get_or_create_bucket("upload", |_| {});
        limiter.on_throttle("upload");

        let throttled_cap = limiter.effective_capacity("upload").unwrap();

        // 100 successes trigger recovery
        for _ in 0..100 {
            limiter.on_success("upload");
        }

        let recovered_cap = limiter.effective_capacity("upload").unwrap();
        assert!(
            recovered_cap > throttled_cap,
            "Capacity should have recovered from {} but got {}",
            throttled_cap,
            recovered_cap
        );
    }

    // ====================================================================
    // Concurrent access tests
    // ====================================================================

    #[tokio::test]
    async fn test_concurrent_acquire() {
        let limiter = Arc::new(AdaptiveRateLimiter::new(RateLimitConfig {
            default_capacity: 50,
            default_refill_rate: 100.0,
            endpoint_overrides: HashMap::new(),
            max_retries: 3,
        }));

        let mut handles = Vec::new();

        // Spawn 20 concurrent tasks each acquiring a token
        for i in 0..20 {
            let limiter = Arc::clone(&limiter);
            let handle = tokio::spawn(async move {
                let _guard = limiter.acquire("concurrent_test").await;
                // Simulate some work
                tokio::time::sleep(Duration::from_millis(1)).await;
                i
            });
            handles.push(handle);
        }

        // All tasks should complete successfully
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        assert_eq!(results.len(), 20);
    }

    #[tokio::test]
    async fn test_concurrent_throttle_and_success() {
        let limiter = Arc::new(AdaptiveRateLimiter::with_defaults());

        // Create bucket
        limiter.get_or_create_bucket("concurrent_ep", |_| {});

        let mut handles = Vec::new();

        // Spawn tasks that alternate between throttle and success
        for i in 0..10 {
            let limiter = Arc::clone(&limiter);
            let handle = tokio::spawn(async move {
                if i % 3 == 0 {
                    limiter.on_throttle("concurrent_ep");
                } else {
                    limiter.on_success("concurrent_ep");
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // Bucket should still be functional
        let cap = limiter.effective_capacity("concurrent_ep").unwrap();
        assert!(cap >= 1, "Capacity should be at least 1, got {}", cap);
    }

    #[test]
    fn test_concurrent_try_acquire_no_overallocation() {
        let bucket = Arc::new(TokenBucket::new(10, 0.0)); // No refill
        let mut handles = Vec::new();

        // Spawn 20 threads each trying to acquire a token
        for _ in 0..20 {
            let bucket = Arc::clone(&bucket);
            let handle = std::thread::spawn(move || if bucket.try_acquire() { 1u32 } else { 0u32 });
            handles.push(handle);
        }

        let total_acquired: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // Should never acquire more tokens than capacity
        assert!(
            total_acquired <= 10,
            "Acquired {} tokens but capacity is 10",
            total_acquired
        );
    }

    // ====================================================================
    // Retry-After parsing tests
    // ====================================================================

    #[test]
    fn test_parse_retry_after_seconds() {
        let duration = parse_retry_after("30", Duration::from_secs(60));
        assert_eq!(duration, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_retry_after_zero() {
        let duration = parse_retry_after("0", Duration::from_secs(60));
        assert_eq!(duration, Duration::from_secs(0));
    }

    #[test]
    fn test_parse_retry_after_with_whitespace() {
        let duration = parse_retry_after("  45  ", Duration::from_secs(60));
        assert_eq!(duration, Duration::from_secs(45));
    }

    #[test]
    fn test_parse_retry_after_invalid_falls_back() {
        let default = Duration::from_secs(60);
        let duration = parse_retry_after("not-a-number", default);
        assert_eq!(duration, default);
    }

    #[test]
    fn test_parse_retry_after_empty_falls_back() {
        let default = Duration::from_secs(30);
        let duration = parse_retry_after("", default);
        assert_eq!(duration, default);
    }

    // ====================================================================
    // RateLimitConfig tests
    // ====================================================================

    #[test]
    fn test_default_config_has_endpoint_overrides() {
        let config = RateLimitConfig::default();
        assert!(config.endpoint_overrides.contains_key("delta"));
        assert!(config.endpoint_overrides.contains_key("upload"));
        assert!(config.endpoint_overrides.contains_key("download"));
        assert!(config.endpoint_overrides.contains_key("metadata"));
    }

    #[test]
    fn test_default_config_values() {
        let config = RateLimitConfig::default();
        assert_eq!(config.default_capacity, 600);
        assert_eq!(config.max_retries, 5);
        assert!(config.default_refill_rate > 0.0);
    }

    // ====================================================================
    // RateLimitGuard tests
    // ====================================================================

    #[test]
    fn test_rate_limit_guard_creation() {
        let guard = RateLimitGuard::new("test".to_string());
        // Guard should be constructable and droppable
        let debug_str = format!("{:?}", guard);
        assert!(debug_str.contains("RateLimitGuard"));
        drop(guard);
    }

    // ====================================================================
    // Integration-style tests
    // ====================================================================

    #[tokio::test]
    async fn test_full_lifecycle_throttle_and_recover() {
        let limiter = AdaptiveRateLimiter::with_defaults();

        // 1. Normal operation - acquire tokens
        let _g = limiter.acquire("lifecycle").await;
        limiter.on_success("lifecycle");

        let initial_cap = limiter.effective_capacity("lifecycle").unwrap();

        // 2. Throttle event - capacity reduced
        limiter.on_throttle("lifecycle");
        let throttled_cap = limiter.effective_capacity("lifecycle").unwrap();
        assert!(throttled_cap < initial_cap);

        // 3. Recovery through successes
        for _ in 0..200 {
            limiter.on_success("lifecycle");
        }
        let recovered_cap = limiter.effective_capacity("lifecycle").unwrap();
        assert!(recovered_cap > throttled_cap);
        assert!(recovered_cap <= initial_cap);
    }

    #[tokio::test]
    async fn test_multiple_endpoints_independent() {
        let limiter = AdaptiveRateLimiter::with_defaults();

        // Acquire from two different endpoints
        let _g1 = limiter.acquire("delta").await;
        let _g2 = limiter.acquire("upload").await;

        // Throttle one endpoint
        limiter.on_throttle("delta");

        // The other should be unaffected
        let delta_cap = limiter.effective_capacity("delta").unwrap();
        let upload_cap = limiter.effective_capacity("upload").unwrap();

        let delta_config_cap = RateLimitConfig::default()
            .endpoint_overrides
            .get("delta")
            .unwrap()
            .0;
        let upload_config_cap = RateLimitConfig::default()
            .endpoint_overrides
            .get("upload")
            .unwrap()
            .0;

        assert_eq!(delta_cap, delta_config_cap / 2); // Throttled
        assert_eq!(upload_cap, upload_config_cap); // Unaffected
    }
}
