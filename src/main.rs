use ferris_cache::{Cache, run_server};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let addr = std::env::var("FERRIS_BIND").unwrap_or_else(|_| "0.0.0.0:7878".to_string());
    let cache: Arc<Cache> = Arc::new(DashMap::new());
    let listener = TcpListener::bind(&addr).await.unwrap();
    println!("ferris-cache listening on {addr}");
    run_server(listener, cache).await;
}
