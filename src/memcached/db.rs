use std::collections::hash_map::DefaultHasher;
use std::sync::Mutex;
use std::usize;
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::Arc,
};

#[derive(Debug, Clone)]
pub struct Item {
    pub key_len: usize,
    pub flags: u32,
    pub rel_exptime: i32,
    pub cas: u64,
    pub data: Box<[u8]>,
}

impl Item {
    pub fn new<K: AsRef<[u8]>,V:AsRef<[u8]>>(key: K, flags: u32, rel_exptime: i32, cas: u64, value: V) -> Item {
        // let mut data = key.as_bytes().to_vec();
        // data.append(&mut value.to_vec());
        let k = key.as_ref();
        let v = value.as_ref();
        let mut data = Vec::with_capacity(k.len() + v.len());
        data.extend_from_slice(k);
        data.extend_from_slice(v);
        // let hash =
        Item {
            key_len: k.len(),
            flags,
            rel_exptime,
            cas,
            data: data.into_boxed_slice(),
        }
    }
}

// 感觉这个shared只需要使用全局static变量就可以了
#[derive(Debug, Clone)]
pub struct Db {
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
    pub fn new(bucket_size: usize) -> Db {
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

    pub fn set(&self, item: &Arc<Item>) -> bool {
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
    pub fn get(&self, item: &Arc<Item>) -> Option<Arc<Item>> {
        let hash = calculate_hash(item);
        let idx = hash as usize % self.shared.buckets.len();
        let mutex = self.shared.buckets.get(idx).unwrap();

        let bucket = mutex.lock().unwrap();
        let res = bucket.entries.get(item);
        res.cloned()
    }

    #[allow(dead_code)]
    pub fn delete(&self, item: &Arc<Item>) -> bool {
        let hash = calculate_hash(item);
        let idx = hash as usize % self.shared.buckets.len();
        let mutex = self.shared.buckets.get(idx).unwrap();

        let mut bucket = mutex.lock().unwrap();

        bucket.entries.remove(item)
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.data[..self.key_len].eq(&other.data[..other.key_len])
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
