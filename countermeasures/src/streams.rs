//! Contains different stream implementations for TCP or TLS streams

use std::{
    io::{self, Read, Write},
    net::Shutdown,
    sync::{Arc, Mutex},
};
use tokio::{net::TcpStream, prelude::*};

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

impl<S> Read for MyStream<S>
where
    S: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use MyStream::*;
        match self {
            Tcp(stream) => stream.read(buf),
            Openssl(stream) => stream.read(buf),
        }
    }
}

impl<S> Write for MyStream<S>
where
    S: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use MyStream::*;
        match self {
            Tcp(stream) => stream.write(buf),
            Openssl(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use MyStream::*;
        match self {
            Tcp(stream) => stream.flush(),
            Openssl(stream) => stream.flush(),
        }
    }
}

impl<S> AsyncRead for MyStream<S> where S: AsyncRead + AsyncWrite {}

impl<S> AsyncWrite for MyStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        use MyStream::*;
        match self {
            Tcp(stream) => stream.shutdown(),
            Openssl(stream) => stream.shutdown(),
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
/// [`AsyncWrite::shutdown`] method which actually calls [`TcpStream::shutdown`] to
/// notify the remote end that we're done writing.
#[derive(Clone, Debug)]
pub struct MyTcpStream(Arc<Mutex<TcpStream>>);

impl MyTcpStream {
    pub fn new(stream: Arc<Mutex<TcpStream>>) -> Self {
        Self(stream)
    }
}

impl Read for MyTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl Write for MyTcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl AsyncRead for MyTcpStream {}

impl AsyncWrite for MyTcpStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.0.lock().unwrap().shutdown(Shutdown::Write)?;
        Ok(().into())
    }
}

/*
/// Wrapper around [`TlsStream`]
///
/// This is a custom type used to have a custom implementation of the
/// [`AsyncWrite::shutdown`] method which actually calls [`TcpStream::shutdown`] to
/// notify the remote end that we're done writing.
#[derive(Debug)]
pub struct TokioRustlsStream<IO, S>(Arc<Mutex<TlsStream<IO, S>>>);

impl<IO, S> TokioRustlsStream<IO, S> {
    pub fn new(stream: Arc<Mutex<TlsStream<IO, S>>>) -> Self {
        Self(stream)
    }
}

impl<IO, S> Clone for TokioRustlsStream<IO, S> {
    fn clone(&self) -> Self {
        TokioRustlsStream(self.0.clone())
    }
}

impl<IO, S> Read for TokioRustlsStream<IO, S>
where
    IO: Read + Write,
    S: Session,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl<IO, S> Write for TokioRustlsStream<IO, S>
where
    IO: Read + Write,
    S: Session,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl<IO, S> AsyncRead for TokioRustlsStream<IO, S>
where
    IO: AsyncRead + AsyncWrite,
    S: Session,
{
}

impl<IO, S> AsyncWrite for TokioRustlsStream<IO, S>
where
    IO: AsyncRead + AsyncWrite,
    S: Session,
{
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.0.lock().unwrap().shutdown()?;
        Ok(().into())
    }
}
*/

/// Wrapper around [`TokioOpensslStream`]
///
/// This is a custom type used to have a custom implementation of the
/// [`AsyncWrite::shutdown`] method which actually calls [`TcpStream::shutdown`] to
/// notify the remote end that we're done writing.
#[derive(Debug)]
pub struct TokioOpensslStream<S>(Arc<Mutex<tokio_openssl::SslStream<S>>>);

impl<S> TokioOpensslStream<S> {
    pub fn new(stream: Arc<Mutex<tokio_openssl::SslStream<S>>>) -> Self {
        Self(stream)
    }
}

impl<S> Clone for TokioOpensslStream<S> {
    fn clone(&self) -> Self {
        TokioOpensslStream(self.0.clone())
    }
}

impl<S> Read for TokioOpensslStream<S>
where
    S: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl<S> Write for TokioOpensslStream<S>
where
    S: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl<S> AsyncRead for TokioOpensslStream<S> where S: AsyncRead + AsyncWrite {}

impl<S> AsyncWrite for TokioOpensslStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.0.lock().unwrap().shutdown()?;
        Ok(().into())
    }
}
