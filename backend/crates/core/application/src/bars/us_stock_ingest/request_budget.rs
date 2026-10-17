use std::time::{Duration, Instant};

const REQUEST_BUDGET_EXHAUSTED: &str = "request_budget_exhausted";
const TIME_BUDGET_EXHAUSTED: &str = "time_budget_exhausted";

pub(super) struct RequestBudget {
    remaining: usize,
    deadline: Instant,
    pub(super) interrupted_reason: Option<&'static str>,
}

impl RequestBudget {
    pub(super) fn new(max_requests: usize, max_duration: Duration, started_at: Instant) -> Self {
        Self::with_deadline_internal(max_requests, started_at + max_duration)
    }

    #[cfg(test)]
    pub(super) fn with_deadline(max_requests: usize, deadline: Instant) -> Self {
        Self::with_deadline_internal(max_requests, deadline)
    }

    fn with_deadline_internal(max_requests: usize, deadline: Instant) -> Self {
        Self {
            remaining: max_requests,
            deadline,
            interrupted_reason: None,
        }
    }

    pub(super) fn remaining(&self) -> usize {
        self.remaining
    }

    pub(super) fn take_at(&mut self, requests: usize, now: Instant) -> bool {
        if !self.can_start_request(now) {
            return false;
        }
        if !self.record(requests) {
            self.interrupted_reason = Some(REQUEST_BUDGET_EXHAUSTED);
            return false;
        }
        true
    }

    pub(super) fn can_start_request(&mut self, now: Instant) -> bool {
        if self.interrupted_reason.is_some() {
            return false;
        }
        if self.remaining == 0 {
            self.interrupted_reason = Some(REQUEST_BUDGET_EXHAUSTED);
            return false;
        }
        if now >= self.deadline {
            self.interrupted_reason = Some(TIME_BUDGET_EXHAUSTED);
            return false;
        }
        true
    }

    pub(super) fn record(&mut self, requests: usize) -> bool {
        if requests > self.remaining {
            return false;
        }
        self.remaining -= requests;
        true
    }

    pub(super) fn refund_request(&mut self) {
        self.remaining = self.remaining.saturating_add(1);
    }

    pub(super) fn mark_time_budget_exhausted(&mut self) {
        self.interrupted_reason = Some(TIME_BUDGET_EXHAUSTED);
    }

    pub(super) fn mark_request_budget_exhausted(&mut self) {
        if self.interrupted_reason.is_none() {
            self.interrupted_reason = Some(REQUEST_BUDGET_EXHAUSTED);
        }
    }

    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }
}
