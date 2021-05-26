use std::{collections::hash_map::DefaultHasher, time::Instant};
use std::sync::Mutex;
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::Arc,
};

use clap::{App, Arg};

#[derive(Debug, Clone)]
struct Item {
    key_len: usize,
    flags: u32,
    rel_exptime: i32,
    cas: u64,
    data: Box<[u8]>,
}

impl Item {
    fn new(key: &[u8], flags: u32, rel_exptime: i32, cas: u64, value: Vec<u8>) -> Item {
        // let mut data = key.as_bytes().to_vec();
        // data.append(&mut value.to_vec());

        let mut data = Vec::with_capacity(key.len() + value.len());
        data.extend_from_slice(key);
        data.extend(value);
        // let hash =
        Item {
            key_len: key.len(),
            flags,
            rel_exptime,
            cas,
            data: data.into_boxed_slice(),
        }
    }
}

// 感觉这个shared只需要使用全局static变量就可以了
#[derive(Debug, Clone)]
struct Db {
    shared: Arc<Shared>,
}
#[derive(Debug)]
struct Shared {
    buckets: Vec<Mutex<Bucket>>,
}
#[derive(Debug, Clone)]
struct Bucket {
    /// Two `Arc`s are equal if their inner values are equal, even if they are
    /// stored in different allocation.
    entries: HashSet<Arc<Item>>,
}

impl Db {
    fn new(bucket_size: usize) -> Db {
        let mut buckets = Vec::with_capacity(bucket_size);
        for _ in 0..bucket_size {
            buckets.push(Mutex::new(Bucket {
                entries: HashSet::<Arc<Item>>::new(),
            }))
        }
        Db {
            shared: Arc::new(Shared { buckets }),
        }
    }

    fn set(&self, item: &Arc<Item>) -> bool {
        // let mutex = self.shared.buckets
        let hash = calculate_hash(item);
        let idx = hash as usize % self.shared.buckets.len();
        let mutex = self.shared.buckets.get(idx).unwrap();

        let mut bucket = mutex.lock().unwrap();
        let exist = bucket.entries.contains(item);
        if exist {
            bucket.entries.remove(item);
        }
        bucket.entries.insert(item.clone());

        exist
    }

    #[allow(dead_code)]
    fn get(&self, item: &Arc<Item>) -> Option<Arc<Item>> {
        let hash = calculate_hash(item);
        let idx = hash as usize % self.shared.buckets.len();
        let mutex = self.shared.buckets.get(idx).unwrap();

        let bucket = mutex.lock().unwrap();
        let res = bucket.entries.get(item);
        res.cloned()
    }

    #[allow(dead_code)]
    fn delete(&self, item: &Arc<Item>) -> bool {
        let hash = calculate_hash(item);
        let idx = hash as usize % self.shared.buckets.len();
        let mutex = self.shared.buckets.get(idx).unwrap();

        let mut bucket = mutex.lock().unwrap();

        bucket.entries.remove(item)
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.data[..self.key_len] == other.data[..other.key_len]
    }
}

impl Eq for Item {}

impl Hash for Item {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data[..self.key_len].hash(state);
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

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
    println!("pid = {}", std::process::id());
    println!("items = {}", cli.items);
    println!("key_len = {}", cli.key_len);
    println!("val_len = {}", cli.val_len);
    let now = Instant::now();
    let mut key = [0u8; 256];
    let random_vals: [u8; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

    let db = Db::new(1024);

    for i in 0..cli.items {
        let k = format!("{:0>1$}", i, cli.key_len);
        unsafe { std::ptr::copy_nonoverlapping(k.as_ptr(), key.as_mut_ptr(), cli.key_len) }
        // println!("{}", std::str::from_utf8(&key[0..cli.key_len])?);
        let val = vec![random_vals[i % 10]; cli.val_len];
        let item = Arc::new(Item::new(&key[..cli.val_len], 0, 0, 0, val));
        let exist = db.set(&item);
        assert!(!exist);
    }
    let elapsed = now.elapsed().as_secs_f64();
    println!("{} seconds", elapsed);

    #[cfg(unix)]
    process_info();
    // println!("{:0>1$}", 5,4);
    std::thread::sleep(std::time::Duration::from_secs(30));
    Ok(())
}

#[cfg(unix)]
fn process_info() {
    use lpfs::pid::stat_self;
    let stat = stat_self().unwrap();
    println!("vss:{}", stat.vsize());
    println!("rss:{}", stat.rss());
}