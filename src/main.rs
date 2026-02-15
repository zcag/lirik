use rspotify::{prelude::*, scopes, AuthCodeSpotify, Config, Credentials, OAuth};
use std::io::{Read, Write};
use std::net::TcpListener;

fn extract_code(request: &str) -> Option<String> {
    let path = request.lines().next()?.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    query
        .split('&')
        .find_map(|p| p.strip_prefix("code="))
        .map(|s| s.to_string())
}

async fn authenticate(spotify: &AuthCodeSpotify) {
    // try cached token first
    let has_token = match spotify.read_token_cache(true).await {
        Ok(Some(token)) => {
            *spotify.token.lock().await.unwrap() = Some(token);
            // check if it actually works (might be expired without refresh token)
            spotify.current_playing(None, None::<Vec<_>>).await.is_ok()
        }
        _ => false,
    };
    if has_token {
        return;
    }

    let url = spotify.get_authorize_url(false).unwrap();
    let listener = TcpListener::bind("127.0.0.1:8888").expect("failed to bind :8888");

    eprintln!("opening browser for spotify auth...");
    open::that(&url).ok();

    let (mut stream, _) = listener.accept().unwrap();
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap();
    let request = String::from_utf8_lossy(&buf[..n]);

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body><h3>done, you can close this tab</h3></body></html>";
    stream.write_all(response.as_bytes()).ok();

    let code = extract_code(&request).expect("no auth code in callback");
    spotify.request_token(&code).await.unwrap();
    spotify.write_token_cache().await.unwrap();
}

#[tokio::main]
async fn main() {
    let creds = Credentials::from_env()
        .expect("set RSPOTIFY_CLIENT_ID and RSPOTIFY_CLIENT_SECRET");
    let oauth = OAuth::from_env(scopes!(
        "user-read-currently-playing",
        "user-read-playback-state"
    ))
    .expect("set RSPOTIFY_REDIRECT_URI=http://127.0.0.1:8888/callback");

    let config = Config {
        token_cached: true,
        token_refreshing: true,
        ..Default::default()
    };

    let spotify = AuthCodeSpotify::with_config(creds, oauth, config);
    authenticate(&spotify).await;

    match spotify.current_playing(None, None::<Vec<_>>).await {
        Ok(Some(ctx)) => {
            let progress = ctx.progress.unwrap_or_default();
            let prog_s = progress.num_seconds();
            let prog_ms = progress.num_milliseconds() % 1000;

            match ctx.item {
                Some(rspotify::model::PlayableItem::Track(track)) => {
                    let artists: Vec<_> =
                        track.artists.iter().map(|a| a.name.as_str()).collect();
                    let dur = track.duration.num_seconds();

                    println!("{} - {}", artists.join(", "), track.name);
                    println!(
                        "{}:{:02}.{:03} / {}:{:02}",
                        prog_s / 60,
                        prog_s % 60,
                        prog_ms,
                        dur / 60,
                        dur % 60
                    );
                    if ctx.is_playing {
                        println!("▶ playing");
                    } else {
                        println!("⏸ paused");
                    }
                }
                Some(rspotify::model::PlayableItem::Episode(ep)) => {
                    println!("{} (podcast)", ep.name);
                    println!(
                        "at {}:{:02}.{:03}",
                        prog_s / 60,
                        prog_s % 60,
                        prog_ms
                    );
                }
                None => println!("playing but no track info available"),
            }
        }
        Ok(None) => println!("nothing playing right now"),
        Err(e) => eprintln!("error: {e}"),
    }
}
