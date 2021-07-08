use clap::{App, Arg};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> io::Result<()> {
    let matches = App::new("proxy")
        .version("0.1")
        .author("Zeke M. <zekemedley@gmail.com>")
        .about("A simple tcp proxy")
        .arg(
            Arg::with_name("client")
                .short("c")
                .long("client")
                .value_name("ADDRESS")
                .help("The address of the client that we will be proxying traffic for")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .value_name("ADDRESS")
                .help("The address of the server that we will be proxying traffic for")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let client = matches.value_of("client").unwrap();
    let server = matches.value_of("server").unwrap();

    proxy(client, server).await
}

#[derive(Debug)]
struct CopyBuf {
    read_done: bool,
    pos: usize,
    cap: usize,
    amt: u64,
    raw_buf: Box<[u8]>,
}

impl CopyBuf {
    fn new() -> Self {
        Self {
            read_done: false,
            pos: 0,
            cap: 0,
            amt: 0,
            raw_buf: vec![0; 2048].into_boxed_slice(),
        }
    }
}

#[derive(Debug)]
struct Tunnel<R: AsyncRead, W: AsyncWrite> {
    reader: R,
    writer: W,
    buf: CopyBuf,
}

impl<'a, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> Tunnel<R, W> {
    fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            buf: CopyBuf::new(),
        }
    }

    async fn copy(&mut self) -> io::Result<()> {
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            if self.buf.pos == self.buf.cap && !self.buf.read_done {
                let n = self.reader.read(&mut *self.buf.raw_buf).await?;
                if n == 0 {
                    self.buf.read_done = true;
                } else {
                    self.buf.pos = 0;
                    self.buf.cap = n;
                }
            }

            // If our buffer has some data, let's write it out!
            while self.buf.pos < self.buf.cap {
                let n = self
                    .writer
                    .write(&self.buf.raw_buf[self.buf.pos..self.buf.cap])
                    .await?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "write zero byte into writer",
                    ));
                } else {
                    self.buf.pos += n;
                    self.buf.amt += n as u64;
                }
            }

            // If we've written all the data and we've seen EOF, flush out the
            // data and finish the transfer.
            if self.buf.pos == self.buf.cap {
                println!("{} bytes data copied.", self.buf.amt);
                if self.buf.read_done {
                    self.writer.flush().await?;
                    return Ok(());
                }
            }
        }
    }
}

async fn proxy(client: &str, server: &str) -> io::Result<()> {
    let listener = TcpListener::bind(client).await?;
    loop {
        let (client, _) = listener.accept().await?;
        let server = TcpStream::connect(server).await?;

        let (eread, ewrite) = client.into_split();
        let (oread, owrite) = server.into_split();

        let mut tunnel1 = Tunnel::new(eread, owrite);
        let mut tunnel2 = Tunnel::new(oread, ewrite);

        let e2o = tokio::spawn(async move { tunnel1.copy().await });
        let o2e = tokio::spawn(async move { tunnel2.copy().await });

        // let e2o = tokio::spawn(async move { io::copy(&mut eread, &mut owrite).await });
        // let o2e = tokio::spawn(async move { io::copy(&mut oread, &mut ewrite).await });

        // let e2o = io::copy(&mut eread, &mut owrite);
        // let o2e = io::copy(&mut oread, &mut ewrite);

        tokio::select! {
            _ = e2o => println!("c2s done"),
            _ = o2e => println!("s2c done"),

        }
    }
}
