use dashmap::DashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};

pub type Cache = DashMap<String, String>;

pub async fn run_server(listener: TcpListener, cache: Arc<Cache>) {
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
                if let Some(val) = value {
                    handle_set(cache, key, val)
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
        Some(val) => format!("{}\r\n", val.value()),
        None => "NIL\r\n".to_string(),
    }
}

fn handle_set(cache: &Cache, key: &str, value: &str) -> String {
    cache.insert(key.to_string(), value.to_string());
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

    #[test]
    fn get_missing_key_returns_nil() {
        let cache = Cache::new();
        assert_eq!(handle_get(&cache, "missing"), "NIL\r\n");
    }

    #[test]
    fn get_existing_key_returns_value() {
        let cache = Cache::new();
        cache.insert("name".to_string(), "ferris".to_string());
        assert_eq!(handle_get(&cache, "name"), "ferris\r\n");
    }

    #[test]
    fn set_inserts_key_and_returns_ok() {
        let cache = Cache::new();
        assert_eq!(handle_set(&cache, "lang", "rust"), "OK\r\n");
        assert_eq!(cache.get("lang").as_deref().map(|v| v.as_str()), Some("rust"));
    }

    #[test]
    fn set_overwrites_existing_value() {
        let cache = Cache::new();
        handle_set(&cache, "key", "first");
        handle_set(&cache, "key", "second");
        assert_eq!(handle_get(&cache, "key"), "second\r\n");
    }

    #[test]
    fn delete_missing_key_returns_nil() {
        let cache = Cache::new();
        assert_eq!(handle_delete(&cache, "ghost"), "NIL\r\n");
    }

    #[test]
    fn delete_existing_key_returns_ok_and_removes_it() {
        let cache = Cache::new();
        cache.insert("tmp".to_string(), "val".to_string());
        assert_eq!(handle_delete(&cache, "tmp"), "OK\r\n");
        assert_eq!(handle_get(&cache, "tmp"), "NIL\r\n");
    }

    #[test]
    fn set_preserves_multiword_value() {
        let cache = Cache::new();
        handle_set(&cache, "msg", "hello world");
        assert_eq!(handle_get(&cache, "msg"), "hello world\r\n");
    }

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
