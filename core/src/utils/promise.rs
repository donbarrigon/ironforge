use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use tokio::task::JoinHandle;

use crate::ForgeError;

pub struct Promise<T> {
    inner: JoinHandle<Result<T, ForgeError>>,
}

impl<T: Send + 'static> Promise<T> {
    /// new creates a new Promise from Future
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

    pub async fn all<F>(fut: Vec<F>) -> Result<Vec<T>, ForgeError>
    where
        F: Future<Output = Result<T, ForgeError>> + Send + 'static,
    {
        let h: Vec<JoinHandle<Result<T, ForgeError>>> = fut.into_iter().map(|f| tokio::spawn(f)).collect();

        let res = futures::future::try_join_all(h).await.map_err(|e| {
            ForgeError::internal()
                .message(format!("one of the tasks in Promise::all did not complete normally"))
                .caused_by(e)
        })?;

        res.into_iter().collect::<Result<Vec<T>, ForgeError>>()

        // let result = futures::future::join_all(h).await;
        // let mut ret = Vec::with_capacity(result.len());
        // for r in result {
        //     match r {
        //         Ok(Ok(v)) => ret.push(v),
        //         Ok(Err(e)) => return Err(e),
        //         Err(e) => return Err(ForgeError::internal().message("task failed").caused_by(e)),
        //     }
        // }
        // Ok(ret)
    }

    pub async fn all_selected<F>(fut: Vec<F>) -> Vec<Result<T, ForgeError>>
    where
        F: Future<Output = Result<T, ForgeError>> + Send + 'static,
    {
        let h: Vec<JoinHandle<Result<T, ForgeError>>> = fut.into_iter().map(|f| tokio::spawn(f)).collect();

        let res = futures::future::join_all(h).await;

        res.into_iter()
            .map(|r| match r {
                Ok(v) => v,
                Err(e) => Err(ForgeError::internal().message("task failed").caused_by(e)),
            })
            .collect()
    }

    pub async fn race<F>(fut: Vec<F>) -> Result<T, ForgeError>
    where
        F: Future<Output = Result<T, ForgeError>> + Send + 'static,
    {
        let h: Vec<JoinHandle<Result<T, ForgeError>>> = fut.into_iter().map(|f| tokio::spawn(f)).collect();

        let (res, i, _) = futures::future::select_all(h).await;

        match res {
            Ok(v) => v,
            Err(e) => Err(ForgeError::internal()
                .message(format!(
                    "task at index {i} (the first to finish) did not complete normally"
                ))
                .caused_by(e)),
        }
    }

    pub async fn any<F>(fut: Vec<F>) -> Result<T, ForgeError>
    where
        F: Future<Output = Result<T, ForgeError>> + Send + 'static,
    {
        let h: Vec<_> = fut
            .into_iter()
            .map(|f| {
                Box::pin(async move {
                    f.await
                        .map_err(|e| ForgeError::internal().message("task failed").caused_by(e))
                }) as Pin<Box<dyn Future<Output = Result<T, ForgeError>> + Send>>
            })
            .collect();

        futures::future::select_ok(h).await.map(|(v, _r)| v)
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
