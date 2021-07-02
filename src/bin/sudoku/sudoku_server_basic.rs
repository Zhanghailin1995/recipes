use std::future::Future;

use bytes::Buf;
use bytes::BytesMut;
use log::error;
use log::info;
use recipes::shutdown::Shutdown;
use subslice::SubsliceExt;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;
use tokio::runtime::Builder;
use tokio::signal;
use tokio::sync::mpsc;
use tokio::time;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast,
};
fn main() -> anyhow::Result<()> {
    let thread_rt = Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("sudoku-server")
        .enable_io()
        .enable_time()
        .build()?;
    thread_rt.block_on(async move {
        env_logger::init();
        let port = 9981;
        let listener = TcpListener::bind(&format!("0.0.0.0:{}", port)).await;
        info!("sudoku server start listening: {}", port);
        // if let Ok(listener) = listener {
        //     let _ = run(listener, signal::ctrl_c()).await;
        // }
        match listener {
            Ok(l) => {
                let _ = run(l, signal::ctrl_c()).await;
            }
            Err(err) => {
                error!("bind address[0.0.0.0:{}] error, cause: {}", port, err);
            }
        }
    });
    Ok(())
}

#[derive(Error, Debug)]
enum Error {
    #[error("ProtocolError")]
    ProtocolError,
    #[error("IOError: {0}")]
    IOError(#[from] std::io::Error),
    #[error("ConnectionError: {0}")]
    ConnectionError(&'static str),
}

#[derive(Debug)]
struct Listener {
    listener: TcpListener,

    notify_shutdown: broadcast::Sender<()>,

    shutdown_complete_rx: mpsc::Receiver<()>,
    shutdown_complete_tx: mpsc::Sender<()>,
}

impl Listener {
    async fn run(&mut self) -> anyhow::Result<()> {
        info!("accepting inbound connections");

        loop {
            let socket = self.accept().await?;

            let mut handler = Handler {
                connection: Connection::new(socket),
                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),
                _shutdown_complete: self.shutdown_complete_tx.clone(),
            };

            // handler reader
            tokio::spawn(async move {
                if let Err(err) = handler.run().await {
                    error!("read error: {}", err);
                }
            });
        }
    }

    async fn accept(&mut self) -> anyhow::Result<TcpStream> {
        let mut backoff = 1;

        // Try to accept a few times
        loop {
            // Perform the accept operation. If a socket is successfully
            // accepted, return it. Otherwise, save the error.
            match self.listener.accept().await {
                Ok((socket, _peer)) => {
                    // info!("peer: {} connected.", peer);
                    return Ok(socket);
                }
                Err(err) => {
                    if backoff > 64 {
                        // Accept has failed too many times. Return the error.
                        return Err(err.into());
                    }
                }
            }

            // Pause execution until the back off period elapses.
            time::sleep(time::Duration::from_secs(backoff)).await;

            // Double the back off
            backoff *= 2;
        }
    }
}

#[derive(Debug)]
struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}

impl Connection {
    /// Create a new `Connection`, backed by `socket`. Read and write buffers
    /// are initialized.
    pub fn new(socket: TcpStream) -> Connection {
        // let (reader, writer) = tokio::io::split(socket);
        Connection {
            stream: BufWriter::new(socket),
            // writer: Arc::new(Mutex::new(BufWriter::new(writer))),
            // Default to a 4KB read buffer. For the use case of mini redis,
            // this is fine. However, real applications will want to tune this
            // value to their specific use case. There is a high likelihood that
            // a larger read buffer will work better.
            buffer: BytesMut::with_capacity(4 * 1024),
        }
    }

    async fn read_frame(&mut self) -> anyhow::Result<Option<Frame>> {
        loop {
            // Attempt to parse a frame from the buffered data. If enough data
            // has been buffered, the frame is returned.
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            // There is not enough buffered data to read a frame. Attempt to
            // read more data from the socket.
            //
            // On success, the number of bytes is returned. `0` indicates "end
            // of stream".
            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                // The remote closed the connection. For this to be a clean
                // shutdown, there should be no data in the read buffer. If
                // there is, this means that the peer closed the socket while
                // sending a frame.
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(Error::ConnectionError("connection reset by peer").into());
                }
            }
        }
    }

    async fn send_result(&mut self, id: Option<&str>, ans: &str) -> anyhow::Result<()> {
        if let Some(id) = id {
            self.stream.write_all(id.as_bytes()).await?;
            self.stream.write_u8(b':').await?;
        }
        self.stream.write_all(ans.as_bytes()).await?;
        self.stream.write_all(b"\r\n").await?;
        self.stream.flush().await?;
        Ok(())
    }
    // frame id:puzzle\r\n or puzzle\r\n
    fn parse_frame(&mut self) -> anyhow::Result<Option<Frame>> {
        // let mut buf = Cursor::new(&self.buffer[..]);
        let line_end = match self.buffer.find(b"\r\n") {
            Some(end) => end,
            None => return Ok(None),
        };

        let mut parts = self.buffer[..line_end].split(|c| c == &b':');
        // let vec = parts.into_iter().collect();
        let maybe_id_or_puzzle = parts.next().ok_or(Error::ProtocolError)?;
        let maybe_id_or_puzzle = std::str::from_utf8(maybe_id_or_puzzle)?.to_string();
        let maybe_puzzle = parts.next();
        if maybe_puzzle.is_none() {
            return Ok(Some(Frame {
                id: None,
                puzzle: maybe_id_or_puzzle,
            }));
        }
        let puzzle = std::str::from_utf8(maybe_puzzle.unwrap())?.to_string();
        if !parts.next().is_none() {
            return Err(Error::ProtocolError.into());
        }
        self.buffer.advance(line_end + 2);
        Ok(Some(Frame {
            id: Some(maybe_id_or_puzzle),
            puzzle,
        }))
    }
}

#[derive(Debug)]
struct Result {
    id: Option<String>,
    ans: String,
}

#[derive(Debug)]
struct Frame {
    id: Option<String>,
    puzzle: String,
}

#[derive(Debug)]
struct Handler {
    connection: Connection,
    shutdown: Shutdown,
    _shutdown_complete: mpsc::Sender<()>,
}

impl Handler {
    async fn run(&mut self) -> anyhow::Result<()> {
        // As long as the shutdown signal has not been received, try to read a
        // new request frame.
        while !self.shutdown.is_shutdown() {
            let maybe_frame = tokio::select! {
                res = self.connection.read_frame() => res?,
                _ = self.shutdown.recv() => {
                    // If a shutdown signal is received, return from `run`.
                    // This will result in the task terminating.
                    return Ok(());
                }
            };

            // If `None` is returned from `read_frame()` then the peer closed
            // the socket. There is no further work to do and the task can be
            // terminated.
            let frame = match maybe_frame {
                Some(frame) => frame,
                None => return Ok(()),
            };
            // get ans
            let ans = sudoku_resolve(&frame.puzzle);
            // let id = frame.puzzle.clone();
            // let id = frame.id.clone();
            // let ans = task::spawn_blocking(move || {
            //     sudoku_resolve(&frame.puzzle)
            // }).await?;
            
            
            // self.connection.send_result(id.as_deref(), &ans).await?;
            self.connection.send_result(frame.id.as_deref(), &ans).await?;
        }
        Ok(())
    }
}

pub async fn run(listener: TcpListener, shutdown: impl Future) -> anyhow::Result<()> {
    let (notify_shutdown, _) = broadcast::channel(1);
    let (shutdown_complete_tx, shutdown_complete_rx) = mpsc::channel(1);

    let mut server = Listener {
        listener,
        notify_shutdown,
        shutdown_complete_tx,
        shutdown_complete_rx,
    };
    tokio::select! {
        res = server.run() => {
            // If an error is received here, accepting connections from the TCP
            // listener failed multiple times and the server is giving up and
            // shutting down.
            //
            // Errors encountered when handling individual connections do not
            // bubble up to this point.
            if let Err(err) = res {
                error!("failed to accept, cause: {}", err);
            }
        }
        _ = shutdown => {
            // The shutdown signal has been received.
            info!("shutting down");
        }
    }

    let Listener {
        mut shutdown_complete_rx,
        shutdown_complete_tx,
        notify_shutdown,
        ..
    } = server;
    // When `notify_shutdown` is dropped, all tasks which have `subscribe`d will
    // receive the shutdown signal and can exit
    drop(notify_shutdown);
    // Drop final `Sender` so the `Receiver` below can complete
    drop(shutdown_complete_tx);

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv().await;

    Ok(())
}

#[inline]
fn sudoku_resolve(req: &str) -> String {
    recipes::sudoku::sudoku_resolve(&req)
}
