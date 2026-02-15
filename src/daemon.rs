use crate::{lyrics, spotify, web};
use rspotify::model::RepeatState;
use rspotify::prelude::*;
use rspotify::AuthCodeSpotify;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{Notify, RwLock};

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

pub async fn execute_cmd(
    client: &AuthCodeSpotify,
    state: &RwLock<spotify::State>,
    cmd: &str,
    arg: Option<&str>,
) -> Result<(), String> {
    match cmd {
        "pause" => client.pause_playback(None).await.map_err(|e| e.to_string()),
        "play" => client
            .resume_playback(None, None)
            .await
            .map_err(|e| e.to_string()),
        "toggle" => {
            let playing = state
                .read()
                .await
                .now_playing
                .as_ref()
                .map(|n| n.is_playing)
                .unwrap_or(false);
            if playing {
                client.pause_playback(None).await
            } else {
                client.resume_playback(None, None).await
            }
            .map_err(|e| e.to_string())
        }
        "next" => client.next_track(None).await.map_err(|e| e.to_string()),
        "prev" => client
            .previous_track(None)
            .await
            .map_err(|e| e.to_string()),
        "volume" => {
            let val: u8 = arg
                .ok_or("missing volume value")?
                .parse()
                .map_err(|_| "invalid volume (0-100)")?;
            client.volume(val, None).await.map_err(|e| e.to_string())
        }
        "seek" => {
            let ms: i64 = arg
                .ok_or("missing seek position")?
                .parse()
                .map_err(|_| "invalid seek value")?;
            client
                .seek_track(chrono::Duration::milliseconds(ms), None)
                .await
                .map_err(|e| e.to_string())
        }
        "shuffle" => {
            let current = state
                .read()
                .await
                .now_playing
                .as_ref()
                .map(|n| n.shuffle)
                .unwrap_or(false);
            client
                .shuffle(!current, None)
                .await
                .map_err(|e| e.to_string())
        }
        "repeat" => {
            let current = state
                .read()
                .await
                .now_playing
                .as_ref()
                .map(|n| n.repeat.as_str().to_string())
                .unwrap_or_default();
            let next = match current.as_str() {
                "off" => RepeatState::Context,
                "context" => RepeatState::Track,
                _ => RepeatState::Off,
            };
            client.repeat(next, None).await.map_err(|e| e.to_string())
        }
        other => Err(format!("unknown command: {other}")),
    }
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
    let client = Arc::new(client);
    let repoll = Arc::new(Notify::new());

    // web server
    if web_port > 0 {
        tokio::spawn(web::serve(web_port, state.clone(), client.clone()));
    }

    // poll loop
    let poll_state = state.clone();
    let poll_client = client.clone();
    let poll_notify = repoll.clone();
    let poll_interval = Duration::from_secs(poll_secs);
    let poll_handle = tokio::spawn(async move {
        let mut current_track = String::new();
        loop {
            let np = spotify::now_playing(&poll_client).await;

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

            tokio::select! {
                _ = tokio::time::sleep(poll_interval) => {}
                _ = poll_notify.notified() => {}
            }
        }
    });

    // unix socket
    let accept_handle = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let state = state.clone();
            let client = client.clone();
            let repoll = repoll.clone();

            tokio::spawn(async move {
                let (reader, mut writer) = tokio::io::split(stream);
                let mut buf_reader = BufReader::new(reader);
                let mut line = String::new();

                // read with 100ms timeout — empty/timeout means state request
                let got_line = tokio::time::timeout(
                    Duration::from_millis(100),
                    buf_reader.read_line(&mut line),
                )
                .await;

                let line = line.trim().to_string();

                if line.is_empty() || got_line.is_err() {
                    // state request (backwards compat)
                    let s = state.read().await;
                    let json = serde_json::to_string(&*s).unwrap();
                    let _ = writer.write_all(json.as_bytes()).await;
                    let _ = writer.write_all(b"\n").await;
                } else {
                    // command request
                    let resp = match serde_json::from_str::<serde_json::Value>(&line) {
                        Ok(v) => {
                            let cmd = v["cmd"].as_str().unwrap_or("");
                            let arg = v["arg"].as_str();
                            match execute_cmd(&client, &state, cmd, arg).await {
                                Ok(()) => r#"{"ok":true}"#.to_string(),
                                Err(e) => {
                                    let e = e.replace('"', r#"\""#);
                                    format!(r#"{{"error":"{e}"}}"#)
                                }
                            }
                        }
                        Err(e) => format!(r#"{{"error":"bad json: {e}"}}"#),
                    };
                    let _ = writer.write_all(resp.as_bytes()).await;
                    let _ = writer.write_all(b"\n").await;
                    repoll.notify_one();
                }
                let _ = writer.shutdown().await;
            });
        }
    });

    tokio::select! {
        _ = poll_handle => {}
        _ = accept_handle => {}
    }

    let _ = std::fs::remove_file(SOCK_PATH);
    let _ = std::fs::remove_file(PID_PATH);
}
