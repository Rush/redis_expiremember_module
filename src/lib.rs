use crossbeam::queue::ArrayQueue;
use lazy_static::lazy_static;
use redis_module::{
    redis_module, raw as rawmod, Context, RedisError, RedisResult, RedisString, RedisValue,
    ThreadSafeContext, KeyType, Status, RedisModuleIO,
};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;

#[derive(Clone, Eq, PartialEq)]
struct ExpiringMember {
    expire_at: SystemTime,
    key: String,
    member: String,
}

impl Ord for ExpiringMember {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.expire_at.cmp(&other.expire_at)
    }
}

impl PartialOrd for ExpiringMember {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

struct ExpirationQueue {
    queue: ArrayQueue<ExpiringMember>,
}

impl ExpirationQueue {
    fn new(capacity: usize) -> Self {
        ExpirationQueue {
            queue: ArrayQueue::new(capacity),
        }
    }

    fn add_member(&self, member: ExpiringMember) -> Result<(), ExpiringMember> {
        self.queue.push(member)
    }

    fn try_pop(&self) -> Option<ExpiringMember> {
        self.queue.pop()
    }
}

lazy_static! {
    static ref EXPIRATION_QUEUE: Arc<ExpirationQueue> = Arc::new(ExpirationQueue::new(10000));
    static ref EXPIRATION_TIMES: Mutex<HashMap<String, SystemTime>> = Mutex::new(HashMap::new());
    static ref THREAD_STARTED: AtomicBool = AtomicBool::new(false);
}

fn expiremember(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() != 4 && args.len() != 5 {
        return Err(RedisError::Str("ERR wrong number of arguments for 'expiremember' command"));
    }

    let key = args[1].to_string();
    let member = args[2].to_string();
    let expire_value = args[3].parse_integer()?;
    
    let unit = if args.len() == 5 { args[4].to_string().to_lowercase() } else { "s".to_string() };

    let expire_at = match unit.as_str() {
        "s" => SystemTime::now() + Duration::from_secs(expire_value as u64),
        "ms" => SystemTime::now() + Duration::from_millis(expire_value as u64),
        _ => return Err(RedisError::Str("ERR invalid time unit for 'expiremember' command")),
    };

    let mut expiration_times = EXPIRATION_TIMES.lock().unwrap();
    match expire_value {
        -1 => {
            expiration_times.remove(&(key.clone() + &member));
            return Ok(RedisValue::Integer(0));
        }
        0 => {
            let redis_string_key = ctx.create_string(key.as_bytes());
            let opened_key = ctx.open_key_writable(&redis_string_key);
            match opened_key.key_type() {
                KeyType::Hash => { let _ = opened_key.hash_del(&member); },
                KeyType::ZSet => { 
                    let redis_string_member = ctx.create_string(member.as_bytes());
                    let _ = ctx.call("ZREM", &[&redis_string_key, &redis_string_member]);
                },
                KeyType::Set => { 
                    let redis_string_member = ctx.create_string(member.as_bytes());
                    let _ = ctx.call("SREM", &[&redis_string_key, &redis_string_member]);
                },
                KeyType::Empty => {
                }
                _ => return Err(RedisError::Str("ERR key type not supported for 'expiremember' command")),
            }
            expiration_times.remove(&(key.clone() + &member));
            return Ok(RedisValue::Integer(1));
        }
        _ => {
            expiration_times.insert(key.clone() + &member, expire_at);
        }
    }
    drop(expiration_times);

    let expiring_member = ExpiringMember { expire_at, key, member };
    let _ = EXPIRATION_QUEUE.add_member(expiring_member);

    if !THREAD_STARTED.load(Ordering::SeqCst) {
        start_expiration_thread();
        THREAD_STARTED.store(true, Ordering::SeqCst);
    }

    Ok(RedisValue::Integer(1))
}

fn start_expiration_thread() {
    thread::spawn(move || {
        let thread_ctx = ThreadSafeContext::new();
        let mut heap = BinaryHeap::new();
        loop {
            let now = SystemTime::now();
            let mut members_to_expire = HashMap::new();

            while let Some(member) = EXPIRATION_QUEUE.try_pop() {
                heap.push(Reverse(member));
            }

            while let Some(Reverse(member)) = heap.peek() {
                if member.expire_at > now {
                    break;
                }

                if let Some(&expiration_time) = EXPIRATION_TIMES.lock().unwrap().get(&(member.key.clone() + &member.member)) {
                    if expiration_time == member.expire_at {
                        members_to_expire.entry(member.key.clone())
                                         .or_insert_with(Vec::new)
                                         .push(member.clone());
                    }
                }
                heap.pop();
            }

            if !members_to_expire.is_empty() {
                let ctx: redis_module::ContextGuard = thread_ctx.lock();
                for (key, members) in &members_to_expire {
                    let redis_string_key = ctx.create_string(key.as_bytes());
                    let key = ctx.open_key_writable(&redis_string_key);
                    match key.key_type() {
                        KeyType::Hash => {
                            for member in members {
                                key.hash_del(&member.member);
                            }
                        },
                        KeyType::ZSet => {
                            for member in members {
                                let redis_string_member = ctx.create_string(member.member.as_bytes());
                                let _ = ctx.call("ZREM", &[&redis_string_key, &redis_string_member]);
                            }
                        },
                        KeyType::Set => {
                            for member in members {
                                let redis_string_member = ctx.create_string(member.member.as_bytes());
                                let _ = ctx.call("SREM", &[&redis_string_key, &redis_string_member]);
                            }
                        },
                        _ => continue,
                    }
                }
                drop(ctx);
            }

            thread::sleep(Duration::from_millis(100));
        }
    });
}

#[cfg(not(test))]
redis_module! {
    name: "expiremember",
    version: 1,
    allocator: (redis_module::alloc::RedisAlloc, redis_module::alloc::RedisAlloc),
    data_types: [],
    commands: [
        ["expiremember", expiremember, "", 0, 0, 0],
    ],
}
