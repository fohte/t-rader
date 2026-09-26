use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;

use futures_util::{FutureExt, StreamExt, stream};

/// 借用した接続を使う future を spawn せずに並列実行する。
/// `tokio::spawn` の `'static` 要求を避け、外側のトランザクションも利用できるようにする。
/// 結果は入力順に返し、panic した future はメッセージを `Err` として返す。
pub(crate) async fn map_concurrent<'a, I, F, Fut, O>(
    items: I,
    limit: usize,
    mut map: F,
) -> Vec<Result<O, String>>
where
    I: IntoIterator + 'a,
    I::IntoIter: 'a,
    F: FnMut(I::Item) -> Fut + 'a,
    Fut: Future<Output = O> + 'a,
    O: 'a,
{
    let mut results = stream::iter(items.into_iter().enumerate())
        .map(|(index, item)| {
            let future = map(item);
            async move {
                let result = AssertUnwindSafe(future)
                    .catch_unwind()
                    .await
                    .map_err(panic_message);
                (index, result)
            }
        })
        .buffer_unordered(limit.max(1))
        .collect::<Vec<_>>()
        .await;

    results.sort_unstable_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    let payload = match payload.downcast::<String>() {
        Ok(message) => return *message,
        Err(payload) => payload,
    };

    match payload.downcast::<&'static str>() {
        Ok(message) => (*message).to_string(),
        Err(_) => "non-string panic payload".to_string(),
    }
}
