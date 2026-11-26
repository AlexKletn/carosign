use core::future::Future;
use core::pin::Pin;

/// A future that can be stored in a trade object.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
