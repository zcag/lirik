use rspotify::prelude::*;
use rspotify::AuthCodeSpotify;
use std::io::{Read, Write};
use std::net::TcpListener;

const BIND_ADDR: &str = "127.0.0.1:8888";

fn extract_code(request: &str) -> Option<String> {
    let path = request.lines().next()?.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    query
        .split('&')
        .find_map(|p| p.strip_prefix("code="))
        .map(|s| s.to_string())
}

fn await_callback() -> String {
    let listener = TcpListener::bind(BIND_ADDR).expect("failed to bind :8888");
    let (mut stream, _) = listener.accept().unwrap();

    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap();
    let request = String::from_utf8_lossy(&buf[..n]);

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body><h3>done, you can close this tab</h3></body></html>";
    stream.write_all(response.as_bytes()).ok();

    extract_code(&request).expect("no auth code in callback")
}

pub async fn authenticate(spotify: &AuthCodeSpotify) {
    if let Ok(Some(token)) = spotify.read_token_cache(true).await {
        *spotify.token.lock().await.unwrap() = Some(token);
        if spotify.current_playing(None, None::<Vec<_>>).await.is_ok() {
            return;
        }
    }

    let url = spotify.get_authorize_url(false).unwrap();
    eprintln!("opening browser for spotify auth...");
    open::that(&url).ok();

    let code = await_callback();
    spotify.request_token(&code).await.unwrap();
    spotify.write_token_cache().await.unwrap();
}
