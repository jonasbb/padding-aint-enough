// Code taken from
// https://jsdw.me/posts/rust-asyncawait-preview/

use futures::Future;
use std::future::Future as StdFuture;

/// Converts from an old style Future to a new style one:
#[allow(dead_code)]
pub(crate) fn forward<I, E>(
    f: impl Future<Item = I, Error = E> + Unpin,
) -> impl StdFuture<Output = Result<I, E>> {
    use tokio_async_await::compat::forward::IntoAwaitable;
    f.into_awaitable()
}

/// Converts from a new style Future to an old style one:
#[allow(dead_code)]
pub(crate) fn backward<I, E>(
    f: impl StdFuture<Output = Result<I, E>>,
) -> impl Future<Item = I, Error = E> {
    use tokio_async_await::compat::backward;
    backward::Compat::new(f)
}
