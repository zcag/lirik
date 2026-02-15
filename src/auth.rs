use rspotify::prelude::*;
use rspotify::AuthCodeSpotify;
use std::io::{Read, Write};
use std::net::TcpListener;

const BIND_ADDR: &str = "127.0.0.1:8888";
const CACHE_FILE: &str = ".spotify_token_cache.json";

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

pub async fn status(spotify: &AuthCodeSpotify) {
    // env vars
    let has_id = std::env::var("RSPOTIFY_CLIENT_ID").is_ok();
    let has_secret = std::env::var("RSPOTIFY_CLIENT_SECRET").is_ok();
    let has_redirect = std::env::var("RSPOTIFY_REDIRECT_URI").is_ok();

    println!("env:");
    println!("  RSPOTIFY_CLIENT_ID       {}", if has_id { "set" } else { "missing" });
    println!("  RSPOTIFY_CLIENT_SECRET   {}", if has_secret { "set" } else { "missing" });
    println!("  RSPOTIFY_REDIRECT_URI    {}", if has_redirect { "set" } else { "missing" });

    // token cache
    let cache_exists = std::path::Path::new(CACHE_FILE).exists();
    let token_valid = if cache_exists {
        if let Ok(Some(token)) = spotify.read_token_cache(true).await {
            *spotify.token.lock().await.unwrap() = Some(token);
            spotify.current_playing(None, None::<Vec<_>>).await.is_ok()
        } else {
            false
        }
    } else {
        false
    };

    println!("\ntoken:");
    println!("  cache file   {}", if cache_exists { CACHE_FILE } else { "not found" });
    println!("  status       {}", if token_valid { "valid" } else { "expired or missing" });

    if !has_id || !has_secret || !has_redirect {
        println!("\nsetup:");
        println!("  1. Create an app at https://developer.spotify.com/dashboard");
        println!("  2. Set redirect URI to http://127.0.0.1:8888/callback");
        println!("  3. Export env vars:");
        println!("     export RSPOTIFY_CLIENT_ID=<your-client-id>");
        println!("     export RSPOTIFY_CLIENT_SECRET=<your-client-secret>");
        println!("     export RSPOTIFY_REDIRECT_URI=http://127.0.0.1:8888/callback");
        println!("  4. Run: lirik auth login");
    } else if !token_valid {
        println!("\nrun `lirik auth login` to authenticate");
    }
}

pub async fn login(spotify: &AuthCodeSpotify) {
    let url = spotify.get_authorize_url(false).unwrap();
    eprintln!("opening browser for spotify auth...");
    open::that(&url).ok();

    let code = await_callback();
    spotify.request_token(&code).await.unwrap();
    spotify.write_token_cache().await.unwrap();
    eprintln!("authenticated and token cached");
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
