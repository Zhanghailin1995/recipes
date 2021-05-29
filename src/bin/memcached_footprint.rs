use std::sync::Arc;
use std::usize;

use clap::{App, Arg};
use libc::sysconf;
use libc::_SC_CLK_TCK;
use recipes::memcached::db::Item;
use recipes::MemcachedDb;

struct Cli {
    items: usize,
    key_len: usize,
    val_len: usize,
}

fn parse_command_line() -> Cli {
    let matches = App::new("Memcached footprint")
        .version("0.0.1")
        .author("Hailin Z . <zhanghailin1995@gmail.com>")
        .about("echo server")
        .arg(
            Arg::with_name("items")
                .short("i")
                .help("items")
                .takes_value(true)
                .default_value("1000000"),
        )
        .arg(
            Arg::with_name("keylen")
                .short("k")
                .long("keylen")
                .takes_value(true)
                .default_value("10"),
        )
        .arg(
            Arg::with_name("valuelen")
                .short("v")
                .long("valuelen")
                .takes_value(true)
                .default_value("100"),
        )
        .get_matches();

    let items = matches.value_of("items").unwrap().parse().unwrap();
    let key_len = matches.value_of("keylen").unwrap().parse().unwrap();
    let val_len = matches.value_of("valuelen").unwrap().parse().unwrap();

    Cli {
        items,
        key_len,
        val_len,
    }
}

fn main() -> anyhow::Result<()> {
    let cli = parse_command_line();
    println!("sizeof(Item) = {}", std::mem::size_of::<Item>());
    println!("sizeof(Arc<Item>) = {}", std::mem::size_of::<Arc<Item>>());
    println!("pid = {}", std::process::id());
    println!("items = {}", cli.items);
    println!("key_len = {}", cli.key_len);
    println!("val_len = {}", cli.val_len);
    // let now = Instant::now();
    let mut key = [0u8; 256];
    let mut val = [0u8; 512];
    let random_vals: [u8; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

    let db = MemcachedDb::new(1024);

    for i in 0..cli.items {
        // let k = format!("{:0>1$}", i, cli.key_len);
        unsafe {
            // 先拷贝0过去
            let x = i.to_ne_bytes();
            std::ptr::write_bytes(
                key.as_mut_ptr(),
                0u8,
                cli.key_len - std::mem::size_of::<usize>(),
            );
            let dst = &mut key[cli.key_len - std::mem::size_of::<usize>()] as *mut u8;
            std::ptr::copy_nonoverlapping(x.as_ptr(), dst, std::mem::size_of::<usize>())
        }
        // println!("{}", std::str::from_utf8(&key[0..cli.key_len])?);
        // let val = vec![random_vals[i % 10]; cli.val_len];
        let v = random_vals[i % 10];
        unsafe {
            std::ptr::write_bytes(val.as_mut_ptr(), v, cli.val_len);
        }
        let item = Arc::new(Item::new(&key[..cli.key_len], 0, 0, 0, &val[..cli.val_len]));
        let exist = db.set(&item);
        assert!(!exist);
    }
    // let elapsed = now.elapsed().as_secs_f64();
    // println!("{} seconds", elapsed);

    #[cfg(unix)]
    process_info();
    // println!("{:0>1$}", 5,4);
    // std::thread::sleep(std::time::Duration::from_secs(30));
    Ok(())
}

#[cfg(unix)]
fn process_info() {
    use lpfs::pid::stat_self;
    let stat = stat_self().unwrap();
    println!("vss: {}", stat.vsize());
    println!("rss: {}", stat.rss());
    let ticks = ticks_per_second() as f64;
    println!("opend files: {}, limit: {}", get_opend_files(), getrlimit());
    println!("user time: {}", *stat.utime() as f64 / ticks);
    println!("sys time: {}", *stat.stime() as f64 / ticks);
}

#[cfg(unix)]
fn ticks_per_second() -> i64 {
    match unsafe { sysconf(_SC_CLK_TCK) } {
        -1 => 0,
        x => x,
    }
}

#[cfg(unix)]
fn get_opend_files() -> usize {
    let read_dir = std::fs::read_dir("/proc/self/fd");
    match read_dir {
        Ok(dir) => {
            return dir.into_iter().count();
        }
        Err(e) => {
            println!("get opend files error: {}", e);
            return 0;
        }
    }
}

#[cfg(unix)]
fn getrlimit() -> usize {
    use rlimit::*;
    let (soft, _hard) = getrlimit(Resource::NOFILE).unwrap();
    soft.as_usize()
}

#[cfg(test)]
mod tests {
    use crate::Item;

    #[test]
    fn test_item_eq() {
        let key = [1u8, 2u8, 1u8, 2u8];
        let item1 = Item::new(&key[..2], 0, 0, 0, &key[..]);
        let item2 = Item::new(&key[2..], 0, 0, 0, &key[..]);

        assert_eq!(item1, item2);
    }
}
