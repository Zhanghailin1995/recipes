use std::{
    intrinsics::transmute,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    time::Instant,
    usize,
};

use anyhow::Result;
use clap::{App, Arg};

#[derive(Debug)]
struct Cli {
    hostname: Option<String>, // hostname
    port: usize,              // tcp port
    transmit: bool,           // transmit or receive
    receive: bool,
    length: usize, // buf length
    number: usize, // buf number
}

fn parse_command_line() -> Cli {
    let matches = App::new("TTCP")
        .version("0.0.1")
        .author("Hailin Z . <zhanghailin1995@gmail.com>")
        .about("just for cli test")
        .arg(
            Arg::with_name("transmit")
                .short("t")
                .help("run as transmit")
                .takes_value(true)
                .conflicts_with("receive"),
        )
        .arg(
            Arg::with_name("receive")
                .short("r")
                .long("receive")
                .help("run as receive")
                .conflicts_with("transmit"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("The TCP listening port or connect port to use")
                .default_value("5002")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("length")
                .short("l")
                .long("length")
                .help("buf length")
                .default_value("65536")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("number")
                .short("n")
                .long("number")
                .help("Number of buffers")
                .default_value("8192")
                .takes_value(true),
        )
        .get_matches();

    let port = matches.value_of("port").unwrap().parse::<usize>().unwrap();
    let hostname = matches.value_of("transmit").map(|s| s.to_string());
    let transmit = matches.is_present("transmit");
    if transmit {
        println!(
            "run as transmit, connect to {}:{}",
            hostname.clone().unwrap(),
            port
        );
    }
    let receive = matches.is_present("receive");
    if receive {
        println!("run as receive, listenning at {}", port);
    }

    let buf_length = matches
        .value_of("length")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let buffer_number = matches
        .value_of("number")
        .unwrap()
        .parse::<usize>()
        .unwrap();

    if transmit {
        print!(
            "buf length: {}\nbuf number: {}\n",
            buf_length, buffer_number
        );
    } else {
        println!("accepting...");
    }

    Cli {
        hostname: hostname,
        port,
        transmit,
        receive,
        length: buf_length,
        number: buffer_number,
    }
}

fn main() -> Result<()> {
    let cli = parse_command_line();
    if cli.receive {
        receive(&cli)?
    } else if cli.transmit {
        transmit(&cli)?
    }

    Ok(())
}

fn transmit(cli: &Cli) -> Result<()> {
    let sock_addr = format!("{}:{}", cli.hostname.clone().unwrap(), cli.port);
    let sock_addr = sock_addr
        .parse::<SocketAddr>()
        .expect(&format!("Unable to resolve {}", sock_addr));
    let mut stream =
        TcpStream::connect(sock_addr).expect(&format!("Unable to connect {}", sock_addr));
    stream.set_nodelay(true)?; // tcp nodelay
    println!("connected");
    let now = Instant::now();
    // write session message
    stream.write_all(&(cli.number as i32).to_ne_bytes()[..])?;
    stream.write_all(&(cli.length as i32).to_ne_bytes()[..])?;

    // prepare payload length + data
    let mut buf = vec![0u8; cli.length];
    let random_payload: Vec<char> = "0123456789ABCDEF".chars().collect();
    for i in 0..cli.length {
        buf[i] = random_payload[i % 16] as u8;
    }

    let total_mb = 1.0 * (cli.length as f64) * (cli.number as f64) / (1024 as f64) / (1024 as f64);
    println!("{} MiB in total", total_mb);

    for _ in 0..cli.number {
        // write length
        stream.write_all(&(cli.length as i32).to_ne_bytes()[..])?;
        // write payload
        stream.write_all(&buf[..])?;

        // wait ack
        let mut ack = [0u8; 4];
        stream.read_exact(&mut ack[..])?;
        let ack = i32::from_ne_bytes(ack);
        assert!(ack == cli.length as i32);
    }

    stream.flush()?;
    let elapsed = now.elapsed().as_secs_f64();
    println!("{} seconds", elapsed);
    println!("{} MiB/s", total_mb / elapsed);

    Ok(())
}

fn receive(cli: &Cli) -> Result<()> {
    let sock_addr = format!("127.0.0.1:{}", cli.port);
    let sock_addr = sock_addr
        .parse::<SocketAddr>()
        .expect(&format!("Unable to resolve {}", sock_addr));
    let listener = TcpListener::bind(sock_addr)?;

    let (mut stream, _) = listener.accept()?;

    // recv session message
    let mut session_buf = [0u8; 8];
    stream.read_exact(&mut session_buf[..])?;
    let (number_buf, length_buf): ([u8; 4], [u8; 4]) = unsafe { transmute(session_buf) };

    let number = i32::from_ne_bytes(number_buf);
    let length = i32::from_ne_bytes(length_buf);

    print!(
        "receive buffer length = {}\nreceive number of buffers = {}\n",
        number, length
    );
    let total_mb = 1.0 * (number as f64) * (length as f64) / (1024 as f64) / (1024 as f64);
    println!("{} MiB in total", total_mb);

    let now = Instant::now();

    let mut length_buf = [0u8; 4];
    let mut payload_buf = vec![0u8; length as usize];
    for _ in 0..number {
        stream.read_exact(&mut length_buf[..])?;
        let ack = i32::from_ne_bytes(length_buf);

        // read payload
        stream.read_exact(&mut payload_buf[..])?;
        // send ack
        stream.write_all(&ack.to_ne_bytes()[..])?;
    }

    stream.flush()?;
    let elapsed = now.elapsed().as_secs_f64();
    println!("{} seconds", elapsed);
    println!("{} MiB/s", total_mb / elapsed);

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test1() {
        let mut buf = vec![0u8; 16];
        let random_payload: Vec<char> = "0123456789ABCDEF".chars().collect();
        for i in 0..16 {
            println!("{}", i);
            buf[i] = random_payload[i % 16] as u8;
        }

        // let s = "Hello world!";
        // let my_vec: Vec<char> = s.chars().collect();
        // println!("my_vec[0]: {}", my_vec[0]);
        // println!("my_vec[1]: {}", my_vec[1]);
    }
}
