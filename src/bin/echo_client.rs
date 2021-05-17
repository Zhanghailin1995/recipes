use std::{io::{Read, Write}, net::{Shutdown, TcpStream}, vec};

use anyhow::Result;
use clap::{App, Arg};

#[derive(Debug)]
struct Cli {
    hostname: String, // hostname
    port: usize,      // tcp port
    length: usize,
}

fn parse_command_line() -> Cli {
    let matches = App::new("echo")
        .version("0.0.1")
        .author("Hailin Z . <zhanghailin1995@gmail.com>")
        .about("echo server")
        .arg(
            Arg::with_name("hostname")
                .short("h")
                .help("Specify the echo server hostname")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("receive")
                .help("Specify the echo server listening port")
                .takes_value(true)
                .default_value("3007"),
        )
        .arg(
            Arg::with_name("length")
                .short("l")
                .long("length")
                .help("How many bytes of data will be sent to the echo server")
                .takes_value(true)
                .default_value("1024000"),
        )
        .get_matches();

    let hostname = matches.value_of("hostname").unwrap().to_string();
    let port = matches.value_of("port").unwrap().parse::<usize>().unwrap();
    let length = matches
        .value_of("length")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    Cli {
        hostname,
        port,
        length,
    }
}

fn main() -> Result<()> {
    let cli = parse_command_line();
    let sock_addr = format!("{}:{}", cli.hostname, cli.port);
    println!("connecting to {}", &sock_addr);
    let mut stream =
        TcpStream::connect(&sock_addr).expect(&format!("Unable to connect {}", &sock_addr));
    println!("connected, sending {} bytes", cli.length);
    // stream.set_nodelay(true)?; // tcp nodelay

    let buf = vec![1u8; cli.length];
    stream.write_all(&buf)?;
    println!("{} bytes data had send.", cli.length);
    
    stream.shutdown(Shutdown::Write)?;

    let mut receive_buf = vec![0u8; cli.length];
    
    let recv_count = stream.read_to_end(&mut receive_buf)?;
    println!("received {} bytes", recv_count);
    // stream.read_exact(&mut receive_buf)?;
    if recv_count != cli.length {
        println!("!!! Incomplete response !!!");
    }
    
    Ok(())
}
