//! A local page for `WebFetch`: every request gets `body` as plain text.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Lets `WebFetch` reach the loopback page and return it unsummarized.
pub const WEB_SETTINGS: &str = "[permissions]\nmode = \"bypass\"\n\n[web]\nallow_private_network = true\nfetch_extract = false\n";

/// Serves `body`; returns the page's URL.
pub async fn web_page(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}/changelog")
}
