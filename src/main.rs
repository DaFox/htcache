use service::CacheService;

use clap::{value_parser, Arg, ArgMatches, Command};

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time;

#[macro_use]
extern crate log;

#[allow(unused_macros)]
macro_rules! either {
    ($c:expr, $a:expr, $b:expr) => {{
        if $c {
            $a
        } else {
            $b
        }
    }};
}

type CacheServiceTS = Arc<Mutex<CacheService>>;

// TODO: remove hash function and use hasher for hashmap
// TODO: create a persister tool for the hashmap to write it to disk

#[tokio::main]
async fn main() {
    let options = get_cli_options();
    let cache = Arc::new(Mutex::new(CacheService::new(128)));

    either!(
        options.get_flag("ecs-logging"),
        ecs_logger::init(),
        pretty_env_logger::init()
    );

    let address = options.get_one::<IpAddr>("addr").unwrap();
    let port = options.get_one::<u16>("port").unwrap();

    let server =
        warp::serve(filters::cache_api(cache.clone())).run(SocketAddr::new(*address, *port));

    futures::join!(cache_gc(60, cache.clone()), server);
}

fn get_cli_options() -> ArgMatches {
    Command::new("htcache")
        .about("HTCache - Simple and fast cache with HTTP interface")
        .version("0.1.0")
        .author("Thomas Hamacher")
        .arg(
            Arg::new("addr")
                .short('a')
                .long("addr")
                .num_args(1)
                .required(false)
                .default_value("127.0.0.1")
                .value_parser(value_parser!(IpAddr)),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .num_args(1)
                .required(false)
                .default_value("3030")
                .value_parser(value_parser!(u16)),
        )
        .arg(
            Arg::new("ecs-logging")
                .long("ecs-logging")
                .num_args(0)
                .required(false)
                .help("Enable ECS compatible logging"),
        )
        .get_matches()
}

async fn cache_gc(secs: u64, cache: CacheServiceTS) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(secs));

        loop {
            interval.tick().await;
            info!("Running garbage collection for cache.");
            cache.lock().await.gc();
        }
    })
}

//
//
//
mod service {
    use chrono::{DateTime, Duration, Utc};
    use std::collections::hash_map::DefaultHasher;
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};

    pub struct CacheRecord {
        created: DateTime<Utc>,
        expires: Option<u32>,
        content: String,
        content_type: Option<String>,
    }

    impl CacheRecord {
        fn is_expired(&self) -> bool {
            self.expires.map_or(false, |ttl| {
                (self.created + Duration::seconds(ttl as i64)) < Utc::now()
            })
        }

        pub fn get(&self) -> Option<&String> {
            either!(self.is_expired(), None, Some(&self.content))
        }

        pub fn get_content_type(&self) -> Option<&String> {
            self.content_type.as_ref()
        }

        pub fn get_age(&self) -> i64 {
            (Utc::now() - self.created).num_seconds()
        }
    }

    pub struct CacheService {
        storage: HashMap<u64, CacheRecord>,
        capacity: usize,
    }

    impl CacheService {
        pub fn new(capacity: usize) -> Self {
            Self {
                storage: HashMap::with_capacity(capacity),
                capacity,
            }
        }

        pub fn gc(&mut self) {
            self.storage.retain(|_, record| !record.is_expired());
            self.storage.shrink_to(self.capacity);
        }

        pub fn get(&self, key: &str) -> Option<&CacheRecord> {
            self.storage.get(&Self::hash(key))
        }

        pub fn set(
            &mut self,
            key: &str,
            val: &str,
            ttl: Option<u32>,
            content_type: Option<String>,
        ) {
            self.storage.insert(
                Self::hash(key),
                CacheRecord {
                    created: Utc::now(),
                    expires: ttl,
                    content: val.to_string(),
                    content_type,
                },
            );
        }

        fn hash<T: Hash>(obj: T) -> u64 {
            let mut hasher = DefaultHasher::new();
            obj.hash(&mut hasher);
            hasher.finish()
        }
    }
}

//
// Build the request filter / middleware chain
//
mod filters {
    use super::handlers;
    use crate::CacheServiceTS;
    use bytes::Bytes;
    use warp::Filter;

    pub fn cache_api(
        cache: CacheServiceTS,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        cache_get(cache.clone())
            .or(cache_put(cache.clone()))
            .with(warp::log("api"))
    }

    pub fn cache_get(
        cache: CacheServiceTS,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!(String)
            .and(warp::get())
            .and(warp::any().map(move || cache.clone()))
            .and_then(handlers::cache_get)
    }

    pub fn cache_put(
        cache: CacheServiceTS,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!(String)
            .and(warp::put())
            .and(warp::body::content_length_limit(1024 * 128))
            .and(warp::body::bytes().map(|bytes: Bytes| {
                String::from_utf8(bytes.to_vec()).expect("error converting bytes to &str")
            }))
            .and(warp::header::optional::<String>("content-type"))
            .and(warp::header::optional::<u32>("x-ttl"))
            .and(warp::any().map(move || cache.clone()))
            .and_then(handlers::cache_put)
    }
}

//
// Build the request handlers
//
mod handlers {
    use crate::CacheServiceTS;
    use std::convert::Infallible;
    use warp::http::StatusCode;

    pub async fn cache_get(
        name: String,
        cache: CacheServiceTS,
    ) -> Result<impl warp::Reply, Infallible> {
        if let Some(record) = cache.lock().await.get(name.as_str()) {
            if let Some(content) = record.get() {
                return Ok(warp::http::Response::builder()
                    .status(200)
                    .header(
                        "Content-Type",
                        record
                            .get_content_type()
                            .unwrap_or(&"text/plain".to_string()),
                    )
                    .header("Age", record.get_age())
                    .body(content.to_string())
                    .unwrap());
            }
        }

        Ok(warp::http::Response::builder()
            .status(404)
            .body(String::new())
            .unwrap())
    }

    pub async fn cache_put(
        name: String,
        body: String,
        content_type: Option<String>,
        ttl: Option<u32>,
        cache: CacheServiceTS,
    ) -> Result<impl warp::Reply, Infallible> {
        cache
            .lock()
            .await
            .set(name.as_str(), &body, ttl, content_type);
        Ok(StatusCode::CREATED)
    }
}
