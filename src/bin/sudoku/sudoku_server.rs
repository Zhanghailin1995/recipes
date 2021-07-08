use std::future::Future;
use std::sync::{Arc, Mutex};

use bytes::Buf;
use bytes::BytesMut;
use log::{error, warn};
use log::info;
use recipes::shutdown::Shutdown;
use subslice::SubsliceExt;
use thiserror::Error;
use threadpool::ThreadPool;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;
use tokio::io::ReadHalf;
use tokio::io::WriteHalf;
use tokio::net::TcpSocket;
use tokio::runtime::Builder;
use tokio::signal;
use tokio::sync::mpsc;
use tokio::time;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast,
};

#[cfg(feature = "jemallocator")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;



fn main() -> anyhow::Result<()> {
    env_logger::init();
    info!("pid = {}", std::process::id());
    let thread_rt = Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("sudoku-server")
        .enable_io()
        .enable_time()
        .build()?;
    thread_rt.block_on(async move {
        let port = 9981;
        // let listener = TcpListener::bind(&format!("0.0.0.0:{}", port)).await;
        let addr = (&format!("0.0.0.0:{}", port)).parse().unwrap();

        let socket = TcpSocket::new_v4().unwrap();
        match socket.bind(addr) {
            Ok(_) => {},
            Err(e) => {
                error!("bind addr: {} error: {}", addr, e);
                return;
            },
        };

        let listener = socket.listen(10240);
        info!("sudoku server start listening: {}", port);
        match listener {
            Ok(l) => {
                let _ = run(l, signal::ctrl_c()).await;
            }
            Err(err) => {
                error!("listenning error, cause: {}", err);
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

    pool: Arc<Mutex<ThreadPool>>,

    shutdown_complete_rx: mpsc::Receiver<()>,
    shutdown_complete_tx: mpsc::Sender<()>,
}

impl Listener {
    async fn run(&mut self) -> anyhow::Result<()> {
        info!("accepting inbound connections");

        loop {
            let socket = self.accept().await?;

            let (reader, writer) = tokio::io::split(socket);
            let (tx, rx) = mpsc::channel(500000);
            let mut reader_handler = ReaderHandler {
                result_tx: tx,
                reader: Reader::new(reader),
                pool: self.pool.clone(),
                throttling: false,
                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),
                _shutdown_complete: self.shutdown_complete_tx.clone(),
            };

            // handler reader
            tokio::spawn(async move {
                if let Err(err) = reader_handler.run().await {
                    error!("read error: {}", err);
                }
            });

            let mut writer_handler = WriterHandler {
                result_rx: rx,
                writer: Writer::new(writer),
                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),
                _shutdown_complete: self.shutdown_complete_tx.clone(),
            };

            // handler reader
            tokio::spawn(async move {
                if let Err(err) = writer_handler.run().await {
                    error!("write error: {}", err);
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
                Ok((socket, peer)) => {
                    info!("peer: {} connected.", peer);
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
struct Reader {
    reader: ReadHalf<TcpStream>,
    buffer: BytesMut,
}

// type Writer = Arc<Mutex<BufWriter<WriteHalf<TcpStream>>>>;

impl Reader {
    /// Create a new `Connection`, backed by `socket`. Read and write buffers
    /// are initialized.
    pub fn new(reader: ReadHalf<TcpStream>) -> Reader {
        // let (reader, writer) = tokio::io::split(socket);
        Reader {
            reader: reader,
            // Default to a 4KB read buffer. For the use case of mini redis,
            // this is fine. However, real applications will want to tune this
            // value to their specific use case. There is a high likelihood that
            // a larger read buffer will work better.
            buffer: BytesMut::with_capacity(512),
        }
    }

    async fn read_frame(&mut self) -> anyhow::Result<Option<Frame>> {
        loop {
            // Attempt to parse a frame from the buffered data. If enough data
            // has been buffered, the frame is returned.
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            // tokio::task::yield_now();

            // There is not enough buffered data to read a frame. Attempt to
            // read more data from the socket.
            //
            // On success, the number of bytes is returned. `0` indicates "end
            // of stream".
            // 这里的写法其实就是一个过载保护，如果buffer里面数据满了，是不会再读取数据的
            // 可以使用 buffer.remaining 来判断，如果buffer小于多少了，就可以继续读取，而不是parse_frame
            // 现在的写法就是buffer中数据不够一帧了才继续读取
            if 0 == self.reader.read_buf(&mut self.buffer).await? {
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
struct Writer {
    writer: BufWriter<WriteHalf<TcpStream>>,
}

impl Writer {
    fn new(writer: WriteHalf<TcpStream>) -> Self {
        Self {
            writer: BufWriter::with_capacity(4 * 1024, writer),
        }
    }

    async fn send_result(&mut self, id: Option<&str>, ans: &str) -> anyhow::Result<()> {
        if let Some(id) = id {
            self.writer.write_all(id.as_bytes()).await?;
            self.writer.write_u8(b':').await?;
        }
        self.writer.write_all(ans.as_bytes()).await?;
        self.writer.write_all(b"\r\n").await?;
        self.writer.flush().await?;
        Ok(())
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
struct ReaderHandler {
    result_tx: mpsc::Sender<Result>,
    reader: Reader,
    pool: Arc<Mutex<ThreadPool>>,
    shutdown: Shutdown,
    throttling: bool,
    _shutdown_complete: mpsc::Sender<()>,
}

impl ReaderHandler {
    async fn run(&mut self) -> anyhow::Result<()> {
        // As long as the shutdown signal has not been received, try to read a
        // new request frame.
        while !self.shutdown.is_shutdown() {
            let maybe_frame = tokio::select! {
                res = self.reader.read_frame() => res?,
                _ = self.shutdown.recv() => {
                    // If a shutdown signal is received, return from `run`.
                    // This will result in the task terminating.
                    return Ok(());
                }
            };

            // If `None` is returned from `read_frame()` then the peer closed
            // the socket. There is no further work to do and the task can be
            // terminated.
            // and should shutdown writer
            let frame = match maybe_frame {
                Some(frame) => frame,
                None => return Ok(()),
            };
            {
                let _ = self.result_tx.reserve().await?;
            }
            
            if self.throttling {
                let queued_count = self.pool.lock().unwrap().queued_count();
                if queued_count % 10000 == 0 {
                    warn!("throttling, too many queued task:{}", queued_count);
                }
                if queued_count < 100 {
                    warn!("throttling cancel, queued task:{}", queued_count);
                    self.throttling = false;
                }
                self.result_tx.send(Result {
                    id: frame.id,
                    ans: "server too busy".to_string(),
                }).await?;
                
                continue;
            }
            
            // pool.queued_count()
            // 过载保护，如果线程池堆积任务超过500000，停止进行计算
            let queued_count = self.pool.lock().unwrap().queued_count();
            if queued_count > 100000 {
                self.throttling = true;
                warn!("throttling, too many queued task:{}", queued_count);
                self.result_tx.send(Result {
                    id: frame.id,
                    ans: "server too busy".to_string(),
                }).await?;
            } else {
                let tx = self.result_tx.clone();
                let pool = self.pool.lock().unwrap();
                pool.execute(move || {
                    let ans = sudoku_resolve(&frame.puzzle);
                    let result = Result { id: frame.id, ans };
                    // tx.reserve()
                    if !tx.is_closed() {
                        let send_res = tx.blocking_send(result);
                        match send_res {
                            Ok(_) => {}
                            Err(e) => {
                                error!("send error {}", e);
                            }
                        }
                    }
                });
            }

            

            // self.req_tx.send(frame).await?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct WriterHandler {
    result_rx: mpsc::Receiver<Result>,
    writer: Writer,
    shutdown: Shutdown,
    _shutdown_complete: mpsc::Sender<()>,
}

impl WriterHandler {
    async fn run(&mut self) -> anyhow::Result<()> {
        while !self.shutdown.is_shutdown() {
            let res = tokio::select! {
                res = self.result_rx.recv() => res,
                _ = self.shutdown.recv() => {
                    // If a shutdown signal is received, return from `run`.
                    // This will result in the task terminating.
                    return Ok(());
                }
            };
            // if frame is none, the reader is close by peer
            let res = match res {
                Some(s) => s,
                None => return Ok(()),
            };
            // 应该有个反馈
            self.writer.send_result(res.id.as_deref(), &res.ans).await?;
        }

        Ok(())
    }
}

pub async fn run(listener: TcpListener, shutdown: impl Future) -> anyhow::Result<()> {
    let (notify_shutdown, _) = broadcast::channel(1);
    let (shutdown_complete_tx, shutdown_complete_rx) = mpsc::channel(1);
    let pool = Arc::new(Mutex::new(
        threadpool::Builder::new()
            .num_threads(4)
            .thread_name("sudoku_compute".into())
            // .thread_stack_size(8_000_000)
            .build(),
    ));
    let mut server = Listener {
        listener,
        notify_shutdown,
        pool,
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

