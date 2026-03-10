use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};

pub struct Entry {
    pub value: String,
    pub expires_at: Option<Instant>,
}

pub type Cache = DashMap<String, Entry>;

pub async fn run_server(listener: TcpListener, cache: Arc<Cache>) {
    // Background sweeper: removes expired keys every second
    let sweeper = Arc::clone(&cache);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let now = Instant::now();
            sweeper.retain(|_, entry| entry.expires_at.map_or(true, |exp| now < exp));
        }
    });

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };
        let cache = Arc::clone(&cache);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, &cache).await {
                eprintln!("connection error: {e}");
            }
        });
    }
}

async fn handle_connection(stream: TcpStream, cache: &Cache) -> Result<(), std::io::Error> {
    let (reader, writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        let mut parts = trimmed.splitn(3, ' ');
        let cmd = parts.next().unwrap_or("");
        let key = parts.next().unwrap_or("");
        let value = parts.next();

        let response = match cmd {
            "GET" => handle_get(cache, key),
            "SET" => {
                if let Some(rest) = value {
                    handle_set(cache, key, rest)
                } else {
                    "ERR wrong number of arguments\r\n".to_string()
                }
            }
            "DELETE" => handle_delete(cache, key),
            "FLUSH" => {
                cache.clear();
                "OK\r\n".to_string()
            }
            _ => "ERR unknown command\r\n".to_string(),
        };

        writer.write_all(response.as_bytes()).await?;
        writer.flush().await?;
    }

    Ok(())
}

fn handle_get(cache: &Cache, key: &str) -> String {
    match cache.get(key) {
        Some(entry) => {
            if let Some(exp) = entry.expires_at {
                if Instant::now() >= exp {
                    drop(entry); // release read lock before acquiring write lock
                    cache.remove(key);
                    return "NIL\r\n".to_string();
                }
            }
            format!("{}\r\n", entry.value)
        }
        None => "NIL\r\n".to_string(),
    }
}

fn handle_set(cache: &Cache, key: &str, rest: &str) -> String {
    // If the last space-separated token parses as a u64, treat it as a TTL in seconds.
    // Otherwise the entire rest string is the value, preserving multi-word value support.
    let (value, expires_at) = match rest.rsplit_once(' ') {
        Some((value_part, candidate)) => match candidate.parse::<u64>() {
            Ok(0) => return "ERR TTL must be a positive integer\r\n".to_string(),
            Ok(secs) => (value_part, Some(Instant::now() + Duration::from_secs(secs))),
            Err(_) => (rest, None),
        },
        None => (rest, None),
    };
    cache.insert(key.to_string(), Entry { value: value.to_string(), expires_at });
    "OK\r\n".to_string()
}

fn handle_delete(cache: &Cache, key: &str) -> String {
    match cache.remove(key) {
        Some(_) => "OK\r\n".to_string(),
        None => "NIL\r\n".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(value: &str) -> Entry {
        Entry { value: value.to_string(), expires_at: None }
    }

    // --- GET ---

    #[test]
    fn get_missing_key_returns_nil() {
        let cache = Cache::new();
        assert_eq!(handle_get(&cache, "missing"), "NIL\r\n");
    }

    #[test]
    fn get_existing_key_returns_value() {
        let cache = Cache::new();
        cache.insert("name".to_string(), entry("ferris"));
        assert_eq!(handle_get(&cache, "name"), "ferris\r\n");
    }

    #[test]
    fn get_unexpired_key_returns_value() {
        let cache = Cache::new();
        handle_set(&cache, "k", "val 60");
        assert_eq!(handle_get(&cache, "k"), "val\r\n");
    }

    #[test]
    fn get_expired_key_returns_nil() {
        let cache = Cache::new();
        cache.insert("k".to_string(), Entry {
            value: "v".to_string(),
            expires_at: Some(Instant::now() - Duration::from_secs(1)),
        });
        assert_eq!(handle_get(&cache, "k"), "NIL\r\n");
    }

    // --- SET ---

    #[test]
    fn set_inserts_key_and_returns_ok() {
        let cache = Cache::new();
        assert_eq!(handle_set(&cache, "lang", "rust"), "OK\r\n");
        assert_eq!(cache.get("lang").as_deref().map(|e| e.value.as_str()), Some("rust"));
    }

    #[test]
    fn set_overwrites_existing_value() {
        let cache = Cache::new();
        handle_set(&cache, "key", "first");
        handle_set(&cache, "key", "second");
        assert_eq!(handle_get(&cache, "key"), "second\r\n");
    }

    #[test]
    fn set_preserves_multiword_value() {
        let cache = Cache::new();
        handle_set(&cache, "msg", "hello world");
        assert_eq!(handle_get(&cache, "msg"), "hello world\r\n");
    }

    #[test]
    fn set_with_ttl_returns_ok_and_sets_expiry() {
        let cache = Cache::new();
        assert_eq!(handle_set(&cache, "k", "v 60"), "OK\r\n");
        assert!(cache.get("k").unwrap().expires_at.is_some());
    }

    #[test]
    fn set_with_zero_ttl_returns_error() {
        let cache = Cache::new();
        assert_eq!(handle_set(&cache, "k", "v 0"), "ERR TTL must be a positive integer\r\n");
    }

    // --- DELETE ---

    #[test]
    fn delete_missing_key_returns_nil() {
        let cache = Cache::new();
        assert_eq!(handle_delete(&cache, "ghost"), "NIL\r\n");
    }

    #[test]
    fn delete_existing_key_returns_ok_and_removes_it() {
        let cache = Cache::new();
        cache.insert("tmp".to_string(), entry("val"));
        assert_eq!(handle_delete(&cache, "tmp"), "OK\r\n");
        assert_eq!(handle_get(&cache, "tmp"), "NIL\r\n");
    }

    // --- FLUSH ---

    #[test]
    fn flush_clears_all_keys() {
        let cache = Cache::new();
        handle_set(&cache, "a", "1");
        handle_set(&cache, "b", "2");
        handle_set(&cache, "c", "3");
        cache.clear();
        assert_eq!(handle_get(&cache, "a"), "NIL\r\n");
        assert_eq!(handle_get(&cache, "b"), "NIL\r\n");
        assert_eq!(handle_get(&cache, "c"), "NIL\r\n");
    }
}
