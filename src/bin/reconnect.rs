use std::time::Duration;

use stubborn_io::{ReconnectOptions, StubbornTcpStream, config::DurationIterator};
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() {
    env_logger::init();
    let addr = "localhost:3001";

    // we are connecting to the TcpStream using the default built in options.
    // these can also be customized (for example, the amount of reconnect attempts,
    // wait duration, etc) using the connect_with_options method.
    let opts = ReconnectOptions {
        retries_to_attempt_fn: Box::new(get_standard_reconnect_strategy),
        exit_if_first_connect_fails: false,
        on_connect_callback: Box::new(|| {}),
        on_disconnect_callback: Box::new(|| {}),
        on_connect_fail_callback: Box::new(|| {}),
    };
    let mut tcp_stream = StubbornTcpStream::connect_with_options(addr,opts).await.unwrap();
    // once we acquire the wrapped IO, in this case, a TcpStream, we can
    // call all of the regular methods on it, as seen below
    loop {
        tcp_stream.write_all(b"hello world!\r\n").await.unwrap();
        let _ = tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
    


}

fn get_standard_reconnect_strategy() -> DurationIterator {
    let initial_attempts = vec![
        Duration::from_secs(5),
        Duration::from_secs(10),
        Duration::from_secs(20),
        Duration::from_secs(30),
        Duration::from_secs(40),
        Duration::from_secs(50),
        Duration::from_secs(60),
    ];

    let repeat = std::iter::repeat(Duration::from_secs(60));

    let forever_iterator = initial_attempts.into_iter().chain(repeat);

    Box::new(forever_iterator)
}