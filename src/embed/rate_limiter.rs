//! Token bucket rate limiter for embedding API calls.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::debug;

/// Token bucket rate limiter for controlling API request rates.
///
/// This limiter tracks both requests per minute (RPM) and tokens per minute (TPM).
/// It uses a token bucket algorithm to allow bursts while maintaining average rates.
pub struct RateLimiter {
    /// Maximum requests per minute
    max_rpm: u32,
    /// Maximum tokens per minute
    max_tpm: u32,
    /// Current request tokens available
    request_tokens: f64,
    /// Current token bucket tokens available
    token_tokens: f64,
    /// Maximum request tokens (bucket capacity)
    max_request_tokens: f64,
    /// Maximum token bucket tokens (bucket capacity)
    max_token_tokens: f64,
    /// Request token refill rate (tokens per second)
    request_refill_rate: f64,
    /// Token bucket refill rate (tokens per second)
    token_refill_rate: f64,
    /// Last time the buckets were refilled
    last_refill: Instant,
    /// Minimum delay between requests (in seconds)
    min_delay_secs: f64,
    /// Last request time
    last_request_time: Option<Instant>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// # Arguments
    /// * `max_rpm` - Maximum requests per minute
    /// * `max_tpm` - Maximum tokens per minute
    /// * `min_delay_ms` - Minimum delay between requests in milliseconds
    pub fn new(max_rpm: u32, max_tpm: u32, min_delay_ms: u64) -> Self {
        let max_request_tokens = max_rpm as f64;
        let max_token_tokens = max_tpm as f64;
        let request_refill_rate = max_rpm as f64 / 60.0; // tokens per second
        let token_refill_rate = max_tpm as f64 / 60.0; // tokens per second
        let min_delay_secs = min_delay_ms as f64 / 1000.0;

        debug!(
            "RateLimiter created: max_rpm={}, max_tpm={}, min_delay={}ms",
            max_rpm, max_tpm, min_delay_ms
        );

        Self {
            max_rpm,
            max_tpm,
            request_tokens: max_request_tokens,
            token_tokens: max_token_tokens,
            max_request_tokens,
            max_token_tokens,
            request_refill_rate,
            token_refill_rate,
            last_refill: Instant::now(),
            min_delay_secs,
            last_request_time: None,
        }
    }

    /// Acquire permission to make a request with the given token count.
    ///
    /// This method will wait (async) until both RPM and TPM limits allow the request.
    ///
    /// # Arguments
    /// * `tokens` - Estimated number of tokens for this request
    pub async fn acquire(&mut self, tokens: usize) {
        let tokens_f64 = tokens as f64;

        loop {
            self.refill();

            // Check minimum delay
            let delay_needed = if let Some(last_time) = self.last_request_time {
                let elapsed = last_time.elapsed().as_secs_f64();
                if elapsed < self.min_delay_secs {
                    self.min_delay_secs - elapsed
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Check if we have enough tokens
            let need_request_tokens = 1.0; // Each request costs 1 request token
            let need_token_tokens = tokens_f64;

            if self.request_tokens >= need_request_tokens
                && self.token_tokens >= need_token_tokens
                && delay_needed <= 0.0
            {
                // We have enough tokens, consume them
                self.request_tokens -= need_request_tokens;
                self.token_tokens -= need_token_tokens;
                self.last_request_time = Some(Instant::now());

                debug!(
                    "RateLimiter: acquired {} tokens (remaining: requests={:.1}, tokens={:.0})",
                    tokens, self.request_tokens, self.token_tokens
                );

                return;
            }

            // Calculate wait time
            let mut wait_secs = delay_needed;

            if self.request_tokens < need_request_tokens {
                let request_wait =
                    (need_request_tokens - self.request_tokens) / self.request_refill_rate;
                wait_secs = wait_secs.max(request_wait);
            }

            if self.token_tokens < need_token_tokens {
                let token_wait = (need_token_tokens - self.token_tokens) / self.token_refill_rate;
                wait_secs = wait_secs.max(token_wait);
            }

            // Add small buffer to avoid busy waiting
            wait_secs += 0.01;

            debug!(
                "RateLimiter: waiting {:.2}s for {} tokens (requests={:.1}, tokens={:.0})",
                wait_secs, tokens, self.request_tokens, self.token_tokens
            );

            tokio::time::sleep(Duration::from_secs_f64(wait_secs)).await;
        }
    }

    /// Refill the token buckets based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed_secs = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;

        // Refill request tokens
        self.request_tokens = (self.request_tokens + elapsed_secs * self.request_refill_rate)
            .min(self.max_request_tokens);

        // Refill token tokens
        self.token_tokens =
            (self.token_tokens + elapsed_secs * self.token_refill_rate).min(self.max_token_tokens);
    }

    /// Get current limits for monitoring.
    pub fn get_stats(&self) -> RateLimiterStats {
        RateLimiterStats {
            max_rpm: self.max_rpm,
            max_tpm: self.max_tpm,
            available_requests: self.request_tokens as u32,
            available_tokens: self.token_tokens as u32,
        }
    }
}

/// Statistics for the rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    /// Maximum requests per minute
    pub max_rpm: u32,
    /// Maximum tokens per minute
    pub max_tpm: u32,
    /// Currently available request tokens
    pub available_requests: u32,
    /// Currently available token tokens
    pub available_tokens: u32,
}

/// Thread-safe rate limiter wrapper.
pub type SharedRateLimiter = Arc<Mutex<RateLimiter>>;

/// Create a new shared rate limiter.
pub fn create_rate_limiter(max_rpm: u32, max_tpm: u32, min_delay_ms: u64) -> SharedRateLimiter {
    Arc::new(Mutex::new(RateLimiter::new(max_rpm, max_tpm, min_delay_ms)))
}

/// Estimate the number of tokens in a text.
///
/// This is a rough estimate:
/// - For CJK characters: approximately 1 token per character
/// - For other characters: approximately 4 characters per token
pub fn estimate_tokens(text: &str) -> usize {
    let _bytes = text.len();
    let chars = text.chars().count();

    // Estimate based on character types
    let cjk_chars = text
        .chars()
        .filter(|c| matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3040}'..='\u{309f}' | '\u{30a0}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}'))
        .count();

    let non_cjk_chars = chars - cjk_chars;

    // CJK: ~1 token per char, Others: ~4 chars per token
    let estimated = cjk_chars + (non_cjk_chars / 4).max(1);

    // Ensure minimum of 1 token and add small overhead
    estimated.max(1) + 5
}

/// Estimate total tokens for a batch of texts.
pub fn estimate_batch_tokens(texts: &[&str]) -> usize {
    texts.iter().map(|t| estimate_tokens(t)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let mut limiter = RateLimiter::new(60, 10000, 100); // 60 RPM, 10k TPM, 100ms min delay

        // First request should succeed immediately
        limiter.acquire(100).await;

        // Should have 59 requests and 9900 tokens remaining
        let stats = limiter.get_stats();
        assert_eq!(stats.max_rpm, 60);
        assert!(stats.available_requests <= 60);
    }

    #[test]
    fn test_estimate_tokens() {
        // English text
        let english = "Hello world, this is a test.";
        let tokens = estimate_tokens(english);
        assert!(tokens > 0);
        assert!(tokens <= english.len());

        // Chinese text
        let chinese = "你好世界，这是一个测试。";
        let tokens_cn = estimate_tokens(chinese);
        assert!(tokens_cn >= chinese.chars().count());

        // Empty string should return at least 1
        let empty = "";
        assert_eq!(estimate_tokens(empty), 6); // minimum + overhead
    }

    #[test]
    fn test_estimate_batch_tokens() {
        let texts = vec!["Hello", "World", "Test"];
        let total = estimate_batch_tokens(&texts);
        assert!(total >= 3);
    }
}
