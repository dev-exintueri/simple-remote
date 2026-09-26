use std::time::{Duration, Instant};

/// 코드 추측 시도에 대한 지수 백오프 및 코드 교체 판단.
#[derive(Debug, Clone)]
pub struct AttemptLimiter {
    consecutive_failures: u32,
    failures_on_code: u32,
    wait_until: Option<Instant>,
}

/// `record_failure` 결과. 현재 코드를 교체해야 하는지 알려준다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailureOutcome {
    pub rotate_code: bool,
}

impl Default for AttemptLimiter {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            failures_on_code: 0,
            wait_until: None,
        }
    }
}

impl AttemptLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// `now` 시점에 시도가 허용되는지 확인한다. 대기 중이면 남은 시간을 돌려준다.
    pub fn check(&self, now: Instant) -> Result<(), Duration> {
        match self.wait_until {
            Some(until) if now < until => Err(until - now),
            _ => Ok(()),
        }
    }

    /// 실패를 기록하고 다음 백오프를 계산한다. 코드별 실패가 5번째면 코드 교체를 요청한다.
    pub fn record_failure(&mut self, now: Instant) -> FailureOutcome {
        self.consecutive_failures += 1;
        self.failures_on_code += 1;

        let n = self.consecutive_failures;
        let secs: u64 = if n - 1 >= 17 {
            86_400
        } else {
            (1u64 << (n - 1)).min(86_400)
        };
        self.wait_until = Some(now + Duration::from_secs(secs));

        let rotate_code = self.failures_on_code >= 5;
        if rotate_code {
            self.failures_on_code = 0;
        }
        FailureOutcome { rotate_code }
    }

    /// 성공을 기록한다. 연속 실패 수와 대기를 초기화한다.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.failures_on_code = 0;
        self.wait_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn backoff_doubles_from_one_second() {
        let t0 = Instant::now();
        let mut l = AttemptLimiter::new();
        assert!(l.check(t0).is_ok());
        l.record_failure(t0);
        assert_eq!(l.check(t0), Err(Duration::from_secs(1)));
        assert!(l.check(t0 + Duration::from_secs(1)).is_ok());
        let t1 = t0 + Duration::from_secs(1);
        l.record_failure(t1);
        assert_eq!(l.check(t1), Err(Duration::from_secs(2)));
    }
    #[test]
    fn fifth_failure_rotates_code_but_keeps_backoff() {
        let mut t = Instant::now();
        let mut l = AttemptLimiter::new();
        for i in 1..=5 {
            let out = l.record_failure(t);
            assert_eq!(out.rotate_code, i == 5);
            t += Duration::from_secs(1 << (i - 1));
        }
        let out = l.record_failure(t); // 새 코드의 첫 실패
        assert!(!out.rotate_code);
        assert_eq!(l.check(t), Err(Duration::from_secs(32))); // 6번째 연속 실패 → 2^5
    }
    #[test]
    fn success_resets() {
        let t = Instant::now();
        let mut l = AttemptLimiter::new();
        l.record_failure(t);
        l.record_success();
        assert!(l.check(t).is_ok());
    }
    #[test]
    fn backoff_is_capped() {
        let mut t = Instant::now();
        let mut l = AttemptLimiter::new();
        for _ in 0..40 {
            l.record_failure(t);
            t += Duration::from_secs(86_400);
        }
        l.record_failure(t);
        assert_eq!(l.check(t), Err(Duration::from_secs(86_400)));
    }
}
