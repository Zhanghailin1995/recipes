use std::{
    io::{Read, Write},
    net::{TcpListener},
};

use anyhow::Result;
use clap::{App, Arg};

#[derive(Debug)]
struct Cli {
    hostname: String, // hostname
    port: usize,      // tcp port
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
        .get_matches();

    let hostname = matches.value_of("hostname").unwrap().to_string();
    let port = matches.value_of("port").unwrap().parse::<usize>().unwrap();

    Cli { hostname, port }
}

fn main() -> Result<()> {
    let cli = parse_command_line();
    println!("Listener on port {}", cli.port);
    let mut count = 0;
    let sock_addr = format!("{}:{}", &cli.hostname, cli.port);

    let listener = TcpListener::bind(sock_addr)?;
    loop {
        let (mut stream, _) = listener.accept()?;
        println!("accepted no.{} client", {
            count = count + 1;
            count
        });
        // stream.set_nodelay(true)?;
        std::thread::spawn(move || {
            println!("thread for no.{} client started.", count);
            let mut buf = [0u8; 4096];
            loop {
                let result = stream.read(&mut buf);
                match result {
                    Ok(read_count) => {
                        if read_count == 0 {
                            println!("peer close the {} connection.", count);
                            break;
                        }
                        let _ = stream.write_all(&buf);
                    }
                    Err(err) => {
                        println!("{:?}", err);
                        break;
                    }
                }
            }
        });
    }
}
