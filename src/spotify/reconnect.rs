use std::time::{Duration, Instant};

pub const MAX_ATTEMPTS: usize = 5;
pub const WINDOW: Duration = Duration::from_secs(600);
const DELAYS: [Duration; MAX_ATTEMPTS] = [
    Duration::from_secs(1),
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

#[derive(Debug, Default)]
pub struct Reconnector {
    attempts: Vec<Instant>,
}

impl Reconnector {
    pub fn next_delay(&mut self, now: Instant) -> Option<Duration> {
        self.attempts
            .retain(|&attempt| now.duration_since(attempt) < WINDOW);
        if self.attempts.len() >= MAX_ATTEMPTS {
            return None;
        }
        let delay = DELAYS[self.attempts.len()];
        self.attempts.push(now);
        Some(delay)
    }

    pub fn reset(&mut self) {
        self.attempts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_attempts_get_increasing_delays_then_none() {
        let mut reconnector = Reconnector::default();
        let now = Instant::now();
        let delays: Vec<_> = (0..MAX_ATTEMPTS)
            .map(|_| reconnector.next_delay(now))
            .collect();
        assert_eq!(
            delays,
            [1, 5, 15, 30, 60].map(|seconds| Some(Duration::from_secs(seconds)))
        );
        assert_eq!(reconnector.next_delay(now), None);
    }

    #[test]
    fn budget_comes_back_when_the_window_passes() {
        let mut reconnector = Reconnector::default();
        let start = Instant::now();
        for _ in 0..MAX_ATTEMPTS {
            reconnector.next_delay(start);
        }
        assert_eq!(
            reconnector.next_delay(start + WINDOW - Duration::from_secs(1)),
            None
        );
        assert_eq!(reconnector.next_delay(start + WINDOW), Some(DELAYS[0]));
    }

    #[test]
    fn expiring_one_attempt_keeps_the_rest_of_the_budget() {
        let mut reconnector = Reconnector::default();
        let start = Instant::now();
        for seconds in 0..5 {
            reconnector.next_delay(start + Duration::from_secs(seconds));
        }
        assert_eq!(
            reconnector.next_delay(start + WINDOW - Duration::from_secs(1)),
            None
        );
        assert_eq!(
            reconnector.next_delay(start + WINDOW),
            Some(Duration::from_secs(60))
        );
        assert_eq!(reconnector.next_delay(start + WINDOW), None);
    }

    #[test]
    fn a_successful_attempt_restores_the_whole_budget() {
        let mut reconnector = Reconnector::default();
        let now = Instant::now();
        for _ in 0..MAX_ATTEMPTS {
            reconnector.next_delay(now);
        }
        reconnector.reset();
        assert_eq!(reconnector.next_delay(now), Some(DELAYS[0]));
    }
}
