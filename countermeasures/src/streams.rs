//! Contains different stream implementations for TCP or TLS streams

use futures::io::Error;
use std::{
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

/// Wrapper around different stream implementations, such that they can be used in a function return type
#[derive(Debug)]
pub enum MyStream<S> {
    Tcp(MyTcpStream),
    Openssl(TokioOpensslStream<S>),
}

impl<S> Clone for MyStream<S> {
    fn clone(&self) -> Self {
        use MyStream::*;
        match self {
            Tcp(stream) => Tcp(stream.clone()),
            Openssl(stream) => Openssl(stream.clone()),
        }
    }
}

impl<S> AsyncRead for MyStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        use MyStream::*;
        match self.get_mut() {
            Tcp(stream) => Pin::new(stream).poll_read(cx, buf),
            Openssl(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl<S> AsyncWrite for MyStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        use MyStream::*;
        match self.get_mut() {
            Tcp(stream) => Pin::new(stream).poll_write(cx, buf),
            Openssl(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        use MyStream::*;
        match self.get_mut() {
            Tcp(stream) => Pin::new(stream).poll_flush(cx),
            Openssl(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        use MyStream::*;
        match self.get_mut() {
            Tcp(stream) => Pin::new(stream).poll_shutdown(cx),
            Openssl(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl<S> From<MyTcpStream> for MyStream<S> {
    fn from(stream: MyTcpStream) -> Self {
        MyStream::Tcp(stream)
    }
}

impl<S> From<TokioOpensslStream<S>> for MyStream<S> {
    fn from(stream: TokioOpensslStream<S>) -> Self {
        MyStream::Openssl(stream)
    }
}

/// Wrapper around [`TcpStream`]
///
/// This is a custom type used to have a custom implementation of the
/// [`AsyncWrite::poll_shutdown`] method which actually calls [`TcpStream::shutdown`] to
/// notify the remote end that we're done writing.
#[derive(Clone, Debug)]
pub struct MyTcpStream(Arc<Mutex<TcpStream>>);

impl MyTcpStream {
    pub fn new(stream: Arc<Mutex<TcpStream>>) -> Self {
        Self(stream)
    }
}

impl AsyncRead for MyTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        Pin::new(&mut *self.0.lock().unwrap()).poll_read(cx, buf)
    }
}

impl AsyncWrite for MyTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        Pin::new(&mut *self.0.lock().unwrap()).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut *self.0.lock().unwrap()).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut *self.0.lock().unwrap()).poll_shutdown(cx)
    }
}

/// Wrapper around [`TokioOpensslStream`]
///
/// This is a custom type used to have a custom implementation of the
/// [`AsyncWrite::poll_shutdown`] method which actually calls [`TcpStream::shutdown`] to
/// notify the remote end that we're done writing.
#[derive(Debug)]
pub struct TokioOpensslStream<S>(Arc<Mutex<tokio_openssl::SslStream<S>>>);

impl<S> TokioOpensslStream<S>
where
    S: AsyncWrite + AsyncRead + Unpin,
{
    pub fn new(stream: Arc<Mutex<tokio_openssl::SslStream<S>>>) -> Self {
        Self(stream)
    }
}

impl<S> Clone for TokioOpensslStream<S> {
    fn clone(&self) -> Self {
        TokioOpensslStream(self.0.clone())
    }
}

impl<S> AsyncRead for TokioOpensslStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        Pin::new(&mut *self.0.lock().unwrap()).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for TokioOpensslStream<S>
where
    S: AsyncWrite + AsyncRead + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        Pin::new(&mut *self.0.lock().unwrap()).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut *self.0.lock().unwrap()).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut *self.0.lock().unwrap()).poll_shutdown(cx)
    }
}
