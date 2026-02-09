use std::future::Future;
use std::task::{Poll, Context};
use std::pin::Pin;
use pin_project::pin_project;
use tokio::io::{AsyncRead, ReadBuf, Result as IoResult};

/// This pattern abstracts "why reading stops" into a future, currently this is used
/// for shutdowns, but other stop conditions can be implemented to use it, e.g.,
/// timeout, crashes, byte limits, externel signals, e.t.c
///
/// This assumes any stop conditions are treated as EOFs. See [this code](https://github.com/vectordotdev/vector/blob/master/src/async_read.rs)

pub trait AsyncReadExt: AsyncRead {
    fn read_until_future<F>(self, until: F) -> ReadUntil<Self, F>
    where
        Self: Sized,
        F: Future<Output = ()>,
    {
        ReadUntil {
            reader: self,
            until,
        }
    }
}

impl<S> AsyncReadExt for S
where
    S: AsyncRead {}

#[pin_project]
pub struct ReadUntil<S, F> {
    #[pin]
    reader: S,
    #[pin]
    until: F,
}

impl<S, F> ReadUntil<S, F> {
    pub const fn get_ref(&self) -> &S {
        &self.reader
    }
    pub const fn get_mut(&mut self) -> &mut S {
        &mut self.reader
    }

    pub fn get_pin_ref(self: Pin<&Self>) -> Pin<&S> {
        self.project_ref().reader
    }
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        self.project().reader
    }
}

impl<S, F> AsyncRead for ReadUntil<S, F>
where
    S: AsyncRead,
    F: Future<Output = ()>,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>>
    {
        let this = self.project();

        match this.until.poll(cx) {
            Poll::Ready(_) => Poll::Ready(Ok(())),
            Poll::Pending => this.reader.poll_read(cx, buf),
        }
    }
}
