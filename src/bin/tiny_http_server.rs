use std::io::prelude::*;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8008")?;
    let mut count = 0;
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        count += 1;
        println!("{}", count);
        thread::spawn(|| {
            echo(stream);
        });
    }

    Ok(())
}

fn echo(mut stream: TcpStream) {
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf[..]).unwrap();
    // thread::sleep(std::time::Duration::from_secs(1));
    println!("{}", std::str::from_utf8(&buf[0..n]).unwrap());
    let _ = stream.set_nodelay(true);
    stream.write_all("HTTP/1.0 200 OK\r\nConnection: close\r\nAccept-Ranges: bytes\r\nCache-Control: private, no-cache, no-store, proxy-revalidate, no-transform\r\nContent-Type: text/html\r\nPragma: no-cache \r\nContent-Length:0\r\n".as_bytes()).unwrap();
    stream.flush().unwrap();
    match stream.shutdown(Shutdown::Both) {
        Ok(_) => {},
        Err(e) => {
            println!("{}", e);
        },
    };
}
