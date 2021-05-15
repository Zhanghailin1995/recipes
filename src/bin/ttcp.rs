use std::{io::Write, net::TcpStream, usize};

use anyhow::Result;
use clap::{App, Arg};



fn main() -> Result<()> {
    let matches = App::new("TTCP")
        .version("0.0.1")
        .author("Hailin Z . <zhanghailin1995@gmail.com>")
        .about("just for cli test")
        .arg(
            Arg::with_name("transmit")
                .short("t")
                .help("run as transmit")
                .takes_value(true)
                .default_value("localhost")
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
                .default_value("4")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("number")
                .short("n")
                .long("number")
                .help("Number of buffers")
                .default_value("2")
                .takes_value(true),
        )
        .get_matches();

    let port = matches.value_of("port").unwrap().parse::<usize>().unwrap();
    let hostname = matches.value_of("transmit").unwrap();
    if matches.is_present("transmit") {
        println!("run as transmit, connect to {}:{}", hostname, port);
    }

    if matches.is_present("receive") {
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

    println!("buf length: {}, buf number: {}", buf_length, buffer_number);

    println!("{}", std::mem::size_of::<A>());

    let sock_addr = format!("{}:{}", hostname, port);
    let mut stream = TcpStream::connect(sock_addr).unwrap();
    stream.set_nodelay(true)?; // tcp nodelay

    for _ in 0..buffer_number {
        stream.write_all(&(buf_length as i32).to_be_bytes()[..])?;
        let buf = vec![0_u8; buf_length];
        stream.write_all(&buf[..])?;
    }

    stream.flush()?;

    Ok(())
}
