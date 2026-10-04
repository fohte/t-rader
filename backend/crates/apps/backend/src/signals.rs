use tokio::sync::watch;

pub(super) async fn wait_for_shutdown(mut receiver: watch::Receiver<bool>) {
    let _ = receiver.wait_for(|shutdown| *shutdown).await;
}

pub(super) async fn wait_for_os_shutdown_signal() -> Result<(), std::io::Error> {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            result = ctrl_c => result,
            _ = terminate.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    ctrl_c.await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{sync::watch, time::timeout};

    use super::*;

    #[tokio::test]
    async fn shutdown_signal_reaches_worker_and_server_waiters() {
        let (sender, receiver) = watch::channel(false);
        let mut worker_waiter = Box::pin(wait_for_shutdown(receiver.clone()));
        let mut server_waiter = Box::pin(wait_for_shutdown(receiver));
        let waiters_are_pending = tokio::select! {
            biased;
            _ = &mut worker_waiter => false,
            _ = &mut server_waiter => false,
            _ = tokio::task::yield_now() => true,
        };
        let signal_sent = sender.send(true).is_ok();
        let waiters_completed = timeout(Duration::from_secs(1), async {
            tokio::join!(worker_waiter, server_waiter);
        })
        .await
        .is_ok();

        assert_eq!(
            (waiters_are_pending, signal_sent, waiters_completed),
            (true, true, true)
        );
    }
}
