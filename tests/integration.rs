use dashmap::DashMap;
use ferris_cache::{Cache, run_server};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn start_test_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let cache: Arc<Cache> = Arc::new(DashMap::new());
    tokio::spawn(run_server(listener, cache));
    port
}

async fn send(port: u16, input: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream.write_all(input.as_bytes()).await.unwrap();
    stream.shutdown().await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

#[tokio::test]
async fn set_and_get() {
    let port = start_test_server().await;
    assert_eq!(send(port, "SET name ferris\r\n").await, "OK\r\n");
    assert_eq!(send(port, "GET name\r\n").await, "ferris\r\n");
}

#[tokio::test]
async fn get_missing_key() {
    let port = start_test_server().await;
    assert_eq!(send(port, "GET nope\r\n").await, "NIL\r\n");
}

#[tokio::test]
async fn delete_existing_key() {
    let port = start_test_server().await;
    send(port, "SET del_me gone\r\n").await;
    assert_eq!(send(port, "DELETE del_me\r\n").await, "OK\r\n");
    assert_eq!(send(port, "GET del_me\r\n").await, "NIL\r\n");
}

#[tokio::test]
async fn delete_missing_key() {
    let port = start_test_server().await;
    assert_eq!(send(port, "DELETE ghost\r\n").await, "NIL\r\n");
}

#[tokio::test]
async fn flush_clears_cache() {
    let port = start_test_server().await;
    send(port, "SET x 1\r\n").await;
    send(port, "SET y 2\r\n").await;
    assert_eq!(send(port, "FLUSH\r\n").await, "OK\r\n");
    assert_eq!(send(port, "GET x\r\n").await, "NIL\r\n");
    assert_eq!(send(port, "GET y\r\n").await, "NIL\r\n");
}

#[tokio::test]
async fn multiword_value() {
    let port = start_test_server().await;
    send(port, "SET msg hello world\r\n").await;
    assert_eq!(send(port, "GET msg\r\n").await, "hello world\r\n");
}

#[tokio::test]
async fn overwrite_value() {
    let port = start_test_server().await;
    send(port, "SET k first\r\n").await;
    send(port, "SET k second\r\n").await;
    assert_eq!(send(port, "GET k\r\n").await, "second\r\n");
}

#[tokio::test]
async fn unknown_command() {
    let port = start_test_server().await;
    assert_eq!(send(port, "PING\r\n").await, "ERR unknown command\r\n");
}

#[tokio::test]
async fn set_missing_value() {
    let port = start_test_server().await;
    assert_eq!(
        send(port, "SET keyonly\r\n").await,
        "ERR wrong number of arguments\r\n"
    );
}

#[tokio::test]
async fn pipeline_multiple_commands() {
    let port = start_test_server().await;
    let response = send(port, "SET a 1\r\nSET b 2\r\nGET a\r\nGET b\r\nDELETE a\r\nGET a\r\n").await;
    assert_eq!(response, "OK\r\nOK\r\n1\r\n2\r\nOK\r\nNIL\r\n");
}
