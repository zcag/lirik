use crate::daemon::SOCK_PATH;
use crate::lyrics;
use crate::spotify::{fmt_time, NowPlaying, State};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn spawn_daemon() {
    let exe = std::env::current_exe().unwrap();
    Command::new(exe)
        .arg("--daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn daemon");
}

fn connect() -> Option<UnixStream> {
    if let Ok(stream) = UnixStream::connect(SOCK_PATH) {
        return Some(stream);
    }
    spawn_daemon();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(stream) = UnixStream::connect(SOCK_PATH) {
            return Some(stream);
        }
    }
    None
}

pub fn estimate(state: &State) -> Option<NowPlaying> {
    let mut np = state.now_playing.clone()?;
    if np.is_playing {
        let elapsed = now_ms().saturating_sub(state.fetched_at_ms);
        let estimated = (np.progress_ms + elapsed).min(np.duration_ms);
        np.progress_ms = estimated;
        np.progress = fmt_time(estimated);
    }
    Some(np)
}

pub fn fetch_state() -> State {
    let stream = match connect() {
        Some(s) => s,
        None => {
            eprintln!("failed to connect to daemon");
            std::process::exit(1);
        }
    };
    let reader = BufReader::new(stream);
    let line = match reader.lines().next() {
        Some(Ok(l)) => l,
        _ => {
            eprintln!("failed to read from daemon");
            std::process::exit(1);
        }
    };
    serde_json::from_str(&line).unwrap()
}

fn current_lyric(state: &State, np: &NowPlaying, offset_ms: i64) -> Option<String> {
    let ly = state.lyrics.as_ref()?;
    if !ly.synced {
        return None;
    }
    let adjusted = (np.progress_ms as i64 + offset_ms).max(0) as u64;
    let idx = lyrics::current_line_index(&ly.lines, adjusted)?;
    Some(ly.lines[idx].text.clone())
}

pub fn json(offset_ms: i64) {
    let state = fetch_state();
    let Some(np) = estimate(&state) else {
        println!("null");
        return;
    };

    let lyric = current_lyric(&state, &np, offset_ms);
    let mut out = serde_json::to_value(&np).unwrap();
    out["lyric"] = serde_json::json!(lyric);
    out["lyrics"] = serde_json::json!(state.lyrics);
    println!("{}", serde_json::to_string(&out).unwrap());
}

pub fn send_command(cmd_json: &str) -> Result<String, String> {
    let mut stream = connect().ok_or("failed to connect to daemon")?;
    stream
        .write_all(cmd_json.as_bytes())
        .map_err(|e| format!("write failed: {e}"))?;
    stream
        .write_all(b"\n")
        .map_err(|e| format!("write failed: {e}"))?;
    stream.flush().map_err(|e| format!("flush failed: {e}"))?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|e| format!("shutdown failed: {e}"))?;

    let reader = BufReader::new(stream);
    let line = reader
        .lines()
        .next()
        .ok_or("no response")?
        .map_err(|e| format!("read failed: {e}"))?;

    let v: serde_json::Value = serde_json::from_str(&line).map_err(|e| format!("bad json: {e}"))?;
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        Err(err.to_string())
    } else {
        Ok("ok".to_string())
    }
}

pub fn plain(from_current: bool, reverse: bool, offset_ms: i64) {
    let state = fetch_state();
    let np = estimate(&state);

    let Some(ly) = &state.lyrics else {
        println!("no lyrics found");
        return;
    };

    let start = if from_current && ly.synced {
        let Some(ref np) = np else {
            println!("nothing playing right now");
            return;
        };
        let adjusted = (np.progress_ms as i64 + offset_ms).max(0) as u64;
        lyrics::current_line_index(&ly.lines, adjusted).unwrap_or(0)
    } else {
        0
    };

    let lines = &ly.lines[start..];
    if reverse {
        for line in lines.iter().rev() {
            println!("{}", line.text);
        }
    } else {
        for line in lines {
            println!("{}", line.text);
        }
    }
}
