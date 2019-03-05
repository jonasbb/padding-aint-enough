// Code taken from
// https://jsdw.me/posts/rust-asyncawait-preview/

use futures::Future;
use std::future::Future as StdFuture;

/// Converts from an old style Future to a new style one:
#[allow(dead_code)]
pub fn forward<F, I, E>(f: F) -> impl StdFuture<Output = Result<I, E>>
where
    F: Future<Item = I, Error = E> + Unpin,
{
    use tokio_async_await::compat::forward::IntoAwaitable;
    f.into_awaitable()
}

/// Converts from a new style Future to an old style one:
#[allow(dead_code)]
pub fn backward<F, I, E>(f: F) -> impl Future<Item = I, Error = E>
where
    F: StdFuture<Output = Result<I, E>>,
{
    use tokio_async_await::compat::backward;
    backward::Compat::new(f)
}
