use crate::daemon::SOCK_PATH;
use crate::lyrics;
use crate::spotify::{fmt_time, NowPlaying, State};
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn spawn_daemon() {
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
    let out = serde_json::json!({
        "artist": np.artist,
        "track": np.track,
        "progress_ms": np.progress_ms,
        "progress": np.progress,
        "duration_ms": np.duration_ms,
        "duration": np.duration,
        "is_playing": np.is_playing,
        "lyric": lyric,
        "lyrics": state.lyrics,
    });
    println!("{}", serde_json::to_string(&out).unwrap());
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
