//! Retry helper avec backoff exponentiel borné.

use std::future::Future;
use std::time::Duration;

use tokio::time::sleep;
use tracing::warn;

use crate::error::LlmError;

const BASE_BACKOFF_MS: u64 = 250;
const MAX_BACKOFF_MS: u64 = 8_000;

/// Exécute `op` jusqu'à `max_attempts` fois en cas d'erreur transitoire.
///
/// La fonction `op` reçoit le numéro de tentative (1-based) et doit retourner
/// un `Future` résolvant un `Result<T, LlmError>`. Si l'erreur n'est pas
/// retryable (cf. `LlmError::is_retryable`), on échoue immédiatement.
pub async fn with_retry<F, Fut, T>(max_attempts: u32, mut op: F) -> Result<T, LlmError>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<T, LlmError>>,
{
    let mut attempt: u32 = 0;
    let mut last_err: Option<LlmError> = None;
    while attempt < max_attempts {
        attempt += 1;
        match op(attempt).await {
            Ok(value) => return Ok(value),
            Err(err) => {
                if !err.is_retryable() {
                    return Err(err);
                }
                let backoff = backoff_for(attempt);
                warn!(error = %err, attempt, ?backoff, "tentative LLM en échec, retry");
                last_err = Some(err);
                sleep(backoff).await;
            }
        }
    }
    Err(LlmError::RetryExhausted {
        attempts: max_attempts,
        source: Box::new(last_err.unwrap_or(LlmError::StreamTerminated)),
    })
}

fn backoff_for(attempt: u32) -> Duration {
    let exp = BASE_BACKOFF_MS.saturating_mul(2u64.saturating_pow(attempt.saturating_sub(1)));
    Duration::from_millis(exp.min(MAX_BACKOFF_MS))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let result = with_retry(3, |_| async { Ok::<_, LlmError>(42) })
            .await
            .unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn retries_on_retryable_then_succeeds() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_inner = counter.clone();
        let result = with_retry(3, move |attempt| {
            let counter = counter_inner.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                if attempt < 2 {
                    Err(LlmError::StreamTerminated)
                } else {
                    Ok(attempt)
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(result, 2);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn does_not_retry_on_non_retryable() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_inner = counter.clone();
        let err = with_retry::<_, _, ()>(5, move |_| {
            let counter = counter_inner.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(LlmError::InvalidConfig("bad".into()))
            }
        })
        .await
        .unwrap_err();
        assert!(matches!(err, LlmError::InvalidConfig(_)));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
