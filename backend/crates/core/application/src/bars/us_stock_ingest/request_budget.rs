pub(super) struct RequestBudget {
    remaining: usize,
}

impl RequestBudget {
    pub(super) fn new(max_requests: usize) -> Self {
        Self {
            remaining: max_requests,
        }
    }

    pub(super) fn remaining(&self) -> usize {
        self.remaining
    }

    pub(super) fn take(&mut self, requests: usize) -> bool {
        if requests > self.remaining {
            return false;
        }
        self.remaining -= requests;
        true
    }
}
