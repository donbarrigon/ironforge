use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use tokio::task::JoinHandle;

use crate::ForgeError;

pub struct Promise<T> {
    inner: JoinHandle<Result<T, ForgeError>>,
}

impl<T: Send + 'static> Promise<T> {
    pub fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Result<T, ForgeError>> + Send + 'static,
    {
        return Self {
            inner: tokio::spawn(fut),
        };
    }

    pub fn abort(&self) {
        self.inner.abort();
    }

    pub fn detach(self) {
        drop(self.inner);
    }

    pub fn then<U, Fn, F>(self, f: Fn) -> Promise<U>
    where
        // T: IntoPromiseResult,
        // Fn: FnOnce(<T as IntoPromiseResult>::Success) -> F + Send + 'static,
        U: Send + 'static,
        Fn: FnOnce(T) -> F + Send + 'static,
        F: Future<Output = Result<U, ForgeError>> + Send + 'static,
    {
        return Promise::new(async move {
            let v = self.await?;
            f(v).await
        });
    }

    pub fn catch<Fn, F>(self, f: Fn) -> Promise<T>
    where
        // T: IntoPromiseResult,
        Fn: FnOnce(ForgeError) -> F + Send + 'static,
        // F: Future<Output = <T as IntoPromiseResult>::Success> + Send + 'static,
        F: Future<Output = Result<T, ForgeError>> + Send + 'static,
    {
        return Promise::new(async move {
            match self.await {
                Ok(v) => Ok(v),
                Err(e) => f(e).await,
            }
        });
    }
}

impl<T: Send + 'static> Future for Promise<T> {
    type Output = Result<T, ForgeError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.inner).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(join_err)) => Poll::Ready(Err(ForgeError::internal()
                .message("failed to join promise")
                .caused_by(join_err))),
            Poll::Pending => Poll::Pending,
        }
        // return Pin::new(&mut self.inner)
        //     .poll(cx)
        //     .map_err(|e| ForgeError::internal().message("failed to join promise").caused_by(e));
    }
}

// pub trait IntoPromiseResult {
//     type Success;
//     fn into_promise_result(self) -> Result<Self::Success, ForgeError>;
// }

// impl<T> IntoPromiseResult for Result<T, ForgeError> {
//     type Success = T;
//     fn into_promise_result(self) -> Result<Self::Success, ForgeError> {
//         return self;
//     }
// }
