use std::future::{Future, poll_fn};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::task::Poll;

/// 借用した接続を使う future を spawn せずに並列実行する。
/// `tokio::spawn` の `'static` 要求を避け、外側のトランザクションも利用できるようにする。
/// panic した future は `Err(())` として返し、他の future の処理を続ける。
pub(crate) async fn map_concurrent<'a, I, F, Fut, O>(
    items: I,
    limit: usize,
    mut map: F,
) -> Vec<Result<O, ()>>
where
    I: IntoIterator + 'a,
    I::IntoIter: 'a,
    F: FnMut(I::Item) -> Fut + 'a,
    Fut: Future<Output = O> + 'a,
    O: 'a,
{
    let mut items = items.into_iter();
    let mut futures = Vec::<Pin<Box<Fut>>>::new();
    let mut results = Vec::new();
    let mut exhausted = false;

    poll_fn(|context| {
        loop {
            while !exhausted && futures.len() < limit.max(1) {
                match items.next() {
                    Some(item) => futures.push(Box::pin(map(item))),
                    None => exhausted = true,
                }
            }

            let mut index = 0;
            while index < futures.len() {
                match catch_unwind(AssertUnwindSafe(|| futures[index].as_mut().poll(context))) {
                    Ok(Poll::Ready(output)) => {
                        results.push(Ok(output));
                        futures.swap_remove(index);
                    }
                    Ok(Poll::Pending) => index += 1,
                    Err(_) => {
                        results.push(Err(()));
                        futures.swap_remove(index);
                    }
                }
            }

            if exhausted && futures.is_empty() {
                return Poll::Ready(());
            }
            if futures.is_empty() {
                continue;
            }
            return Poll::Pending;
        }
    })
    .await;

    results
}
