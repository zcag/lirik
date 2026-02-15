use crate::{lyrics, spotify, web};
use rspotify::AuthCodeSpotify;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use tokio::sync::RwLock;

pub const SOCK_PATH: &str = "/tmp/lirik.sock";
pub const PID_PATH: &str = "/tmp/lirik.pid";

pub fn kill() {
    if let Ok(raw) = std::fs::read_to_string(PID_PATH) {
        if let Ok(pid) = raw.trim().parse::<i32>() {
            unsafe { libc::kill(pid, libc::SIGTERM); }
            for _ in 0..20 {
                std::thread::sleep(Duration::from_millis(50));
                if unsafe { libc::kill(pid, 0) } != 0 {
                    break;
                }
            }
            // force kill if still alive
            if unsafe { libc::kill(pid, 0) } == 0 {
                unsafe { libc::kill(pid, libc::SIGKILL); }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    let _ = std::fs::remove_file(SOCK_PATH);
    let _ = std::fs::remove_file(PID_PATH);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub async fn run(client: AuthCodeSpotify, poll_secs: u64, web_port: u16) {
    std::fs::write(PID_PATH, std::process::id().to_string()).ok();
    let _ = std::fs::remove_file(SOCK_PATH);

    let listener = UnixListener::bind(SOCK_PATH).expect("failed to bind unix socket");
    let state: Arc<RwLock<spotify::State>> = Arc::new(RwLock::new(spotify::State {
        now_playing: None,
        fetched_at_ms: now_ms(),
        lyrics: None,
    }));

    // web server
    if web_port > 0 {
        tokio::spawn(web::serve(web_port, state.clone()));
    }

    // poll loop
    let poll_state = state.clone();
    let poll_interval = Duration::from_secs(poll_secs);
    let poll_handle = tokio::spawn(async move {
        let mut current_track = String::new();
        loop {
            let np = spotify::now_playing(&client).await;

            let track_key = np
                .as_ref()
                .map(|n| format!("{}\0{}", n.artist, n.track))
                .unwrap_or_default();

            let ly = if track_key != current_track {
                current_track = track_key;
                match &np {
                    Some(n) => lyrics::fetch(&n.artist, &n.track, n.duration_ms).await,
                    None => None,
                }
            } else {
                poll_state.read().await.lyrics.clone()
            };

            {
                let mut s = poll_state.write().await;
                s.now_playing = np;
                s.fetched_at_ms = now_ms();
                s.lyrics = ly;
            }
            tokio::time::sleep(poll_interval).await;
        }
    });

    // unix socket
    let accept_handle = tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                let s = state.read().await;
                let json = serde_json::to_string(&*s).unwrap();
                let _ = stream.write_all(json.as_bytes()).await;
                let _ = stream.write_all(b"\n").await;
                let _ = stream.shutdown().await;
            }
        }
    });

    tokio::select! {
        _ = poll_handle => {}
        _ = accept_handle => {}
    }

    let _ = std::fs::remove_file(SOCK_PATH);
    let _ = std::fs::remove_file(PID_PATH);
}
