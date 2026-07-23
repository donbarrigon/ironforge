use bytes::{Buf, Bytes};
use futures::Stream;
use hyper::body::{Body, Frame};
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};

use crate::error::HttpError;

pub type BoxStream = Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, HttpError>> + Send>>;

pub enum ResBody {
    Full(Option<Bytes>),
    Stream(BoxStream),
}

impl ResBody {
    pub fn full(bytes: Bytes) -> Self {
        ResBody::Full(Some(bytes))
    }

    pub fn stream(s: BoxStream) -> Self {
        ResBody::Stream(s)
    }
}

impl Body for ResBody {
    type Data = Bytes;
    type Error = HttpError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.get_mut() {
            // Full entrega un único frame y después siempre None.
            // El Option adentro es el "ya lo mandé, no mandes de nuevo".
            ResBody::Full(slot) => match slot.take() {
                Some(bytes) => Poll::Ready(Some(Ok(Frame::data(bytes)))),
                None => Poll::Ready(None),
            },
            ResBody::Stream(s) => s.as_mut().poll_next(cx),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self {
            ResBody::Full(slot) => slot.is_none(),
            ResBody::Stream(_) => false, // no sabemos si terminó sin pollear
        }
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        match self {
            ResBody::Full(Some(b)) => hyper::body::SizeHint::with_exact(b.remaining() as u64),
            ResBody::Full(None) => hyper::body::SizeHint::with_exact(0),
            ResBody::Stream(_) => hyper::body::SizeHint::default(), // desconocido -> chunked
        }
    }
}
