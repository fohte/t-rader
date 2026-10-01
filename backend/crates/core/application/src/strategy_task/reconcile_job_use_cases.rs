use super::reconcile_job_queue::{
    SharedStrategyTaskReconcileJobQueue, StrategyTaskReconcileJobQueueError,
};

#[derive(Clone)]
pub struct StrategyTaskReconcileJobUseCases {
    queue: SharedStrategyTaskReconcileJobQueue,
}

impl StrategyTaskReconcileJobUseCases {
    pub fn new(queue: SharedStrategyTaskReconcileJobQueue) -> Self {
        Self { queue }
    }

    pub async fn enqueue_reconciliation(&self) -> Result<(), StrategyTaskReconcileJobQueueError> {
        self.queue.enqueue_reconciliation().await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;

    use super::*;

    struct FakeQueue {
        enqueues: AtomicUsize,
        error: Option<String>,
    }

    #[async_trait]
    impl super::super::StrategyTaskReconcileJobQueue for FakeQueue {
        async fn enqueue_reconciliation(&self) -> Result<(), StrategyTaskReconcileJobQueueError> {
            self.enqueues.fetch_add(1, Ordering::SeqCst);
            match &self.error {
                Some(message) => Err(StrategyTaskReconcileJobQueueError::new(message.clone())),
                None => Ok(()),
            }
        }
    }

    #[tokio::test]
    async fn enqueues_reconciliation_once() {
        let queue = Arc::new(FakeQueue {
            enqueues: AtomicUsize::new(0),
            error: None,
        });
        let use_cases = StrategyTaskReconcileJobUseCases::new(queue.clone());

        let result = use_cases.enqueue_reconciliation().await;

        assert_eq!((result, queue.enqueues.load(Ordering::SeqCst)), (Ok(()), 1));
    }

    #[tokio::test]
    async fn propagates_queue_failure() {
        let queue = Arc::new(FakeQueue {
            enqueues: AtomicUsize::new(0),
            error: Some("sample failure".to_string()),
        });
        let use_cases = StrategyTaskReconcileJobUseCases::new(queue.clone());

        let result = use_cases
            .enqueue_reconciliation()
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            (result, queue.enqueues.load(Ordering::SeqCst)),
            (
                Err(
                    "failed to enqueue strategy task reconciliation job: sample failure"
                        .to_string()
                ),
                1,
            ),
        );
    }
}
