use anyhow::Result;

use clap::{App, Arg};
use std::{
    intrinsics::transmute,
    net::UdpSocket,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
struct Cli {
    hostname: String,
    server: bool,
    client: bool,
}

const PORT: usize = 3123;

const MSG_SIZE: usize = std::mem::size_of::<Message>();

fn parse_command_line() -> Cli {
    let matches = App::new("echo")
        .version("0.0.1")
        .author("Hailin Z . <zhanghailin1995@gmail.com>")
        .about("echo server")
        .arg(
            Arg::with_name("hostname")
                .short("h")
                .help("Specify the server hostname")
                .takes_value(true)
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("client")
                .short("c")
                .long("client")
                .help("run as client")
                .conflicts_with("server"),
        )
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .help("run as server")
                .conflicts_with("client"),
        )
        .get_matches();

    let hostname = matches.value_of("hostname").unwrap().to_string();
    let server = matches.is_present("server");
    let client = matches.is_present("client");

    Cli {
        hostname,
        server,
        client,
    }
}

fn main() -> Result<()> {
    let cli = parse_command_line();
    if cli.client {
        run_client(&cli)?
    } else if cli.server {
        run_server(&cli)?
    }
    Ok(())
}

#[derive(Debug)]
struct Message {
    req: i64,
    res: i64,
}

fn run_client(cli: &Cli) -> Result<()> {
    let local_addr = "0.0.0.0:0";
    let udp_socket = UdpSocket::bind(local_addr)?;
    let server_addr = format!("{}:{}", &cli.hostname, PORT);
    println!("connecting to {}", &server_addr);
    udp_socket.connect(&server_addr)?;
    println!("connected");
    let udp_socket = Arc::new(udp_socket);

    let socket1 = udp_socket.clone();

    std::thread::spawn(move || -> Result<()> {
        loop {
            let message = Message { req: now(), res: 0 };

            // let msg_buf =  { transmute::<&Message, &[u8; MSG_SIZE]>(&message) }; // update by cargo clippy
            let msg_buf = unsafe { &*(&message as *const Message as *const [u8; MSG_SIZE]) };
            let write_count = socket1.send(msg_buf)?;
            if write_count == 0 {
                println!("send message error");
                break;
            } else if write_count != MSG_SIZE {
                println!(
                    "send message of {} bytes, expect {} bytes.",
                    write_count, MSG_SIZE
                )
            }
            // let ten_millis = time::Duration::from_(10);
            std::thread::sleep(std::time::Duration::from_micros(200 * 1000));
        }
        Ok(())
    });

    loop {
        // tokio 的说法是如果你使用异步的话，最好使用 let mut recv_buf = vec![0;MSG_SIZE];
        // 因为编译器会把一个async fn包装成一个future，如果使用数组，那么这个future会非常大
        // A stack buffer is explicitly avoided. Recall from earlier, we noted that all task
        // data that lives across calls to .await must be stored by the task. In this case, 
        // buf is used across .await calls. All task data is stored in a single allocation. 
        // You can think of it as an enum where each variant is the data that needs to be 
        // stored for a specific call to .await.
        // https://tokio.rs/tokio/tutorial/io
        let mut recv_buf = [0u8; MSG_SIZE];
        // let mut recv_buf = vec![0;MSG_SIZE];
        // let bytes = Bytes::from(recv_buf.to_vec());
        let read_count = udp_socket.recv(&mut recv_buf)?;

        if read_count == MSG_SIZE {
            // let bytes = Bytes::from(recv_buf);
            // let mut message = unsafe { transmute::<[u8; MSG_SIZE], Message>(recv_buf) };
            // let req_buf = &recv_buf[..MSG_SIZE/2];
            // let res_buf = &recv_buf[MSG_SIZE/2..];
            let (req_buf, res_buf): ([u8; MSG_SIZE / 2], [u8; MSG_SIZE / 2]) =
                unsafe { transmute(recv_buf) };
            let now = now();
            // let req = i64::from_ne_bytes(req_buf.try_into().unwrap());
            // let res = i64::from_ne_bytes(res_buf.try_into().unwrap());
            let req = i64::from_ne_bytes(req_buf);
            let res = i64::from_ne_bytes(res_buf);
            let mine = (now + req) / 2;
            println!(
                "now {}, round trip time {} us, clock error {} us",
                now,
                now - req,
                res - mine
            );
        } else if read_count == 0 {
            eprintln!("recv message");
        } else {
            println!(
                "revc message of {} bytes, expect {} bytes.",
                read_count, MSG_SIZE
            );
        }

        // unimplemented!()
    }

    // Ok(())
}

fn run_server(cli: &Cli) -> Result<()> {
    let server_addr = format!("{}:{}", &cli.hostname, PORT);
    let udp_socket = UdpSocket::bind(server_addr)?;

    loop {
        let mut buf = [0u8; MSG_SIZE];
        let (read_count, peer_addr) = udp_socket.recv_from(&mut buf)?;
        if read_count == MSG_SIZE {
            let (_, right) = buf.split_at_mut(MSG_SIZE / 2);
            right.clone_from_slice(&now().to_ne_bytes());
            let write_count = udp_socket.send_to(&buf, peer_addr)?;
            if write_count == 0 {
                println!("send message error");
                break;
            } else if write_count != MSG_SIZE {
                println!(
                    "send message of {} bytes, expect {} bytes.",
                    write_count, MSG_SIZE
                )
            }

            // right.read_u32()
            // maybe use byteorder crate
            // let res = &now().to_ne_bytes() as *const u8;
            // let dst = &mut buf[MSG_SIZE / 2] as *mut u8;
            // unsafe { std::ptr::copy_nonoverlapping(res, dst, MSG_SIZE / 2) }
        } else if read_count == 0 {
            eprintln!("recv message");
        } else {
            println!(
                "revc message of {} bytes, expect {} bytes.",
                read_count, MSG_SIZE
            );
        }
    }
    Ok(())
}

fn now() -> i64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_micros() as i64
}
