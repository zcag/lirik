use crate::{client, lyrics};
use std::time::{Duration, Instant};

pub fn run(json: bool, offset_ms: i64) {
    let mut state = client::fetch_state();
    let mut last_fetch = Instant::now();
    let mut current_track = String::new();
    let mut last_idx: Option<usize> = None;
    let mut was_playing = true;

    loop {
        if last_fetch.elapsed() > Duration::from_secs(2) {
            state = client::fetch_state();
            last_fetch = Instant::now();
        }

        let Some(np) = client::estimate(&state) else {
            if was_playing {
                was_playing = false;
                println!();
            }
            current_track.clear();
            last_idx = None;
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };

        if !np.is_playing {
            if was_playing {
                was_playing = false;
                current_track.clear();
                last_idx = None;
                println!();
            }
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }

        was_playing = true;

        let track_key = format!("{}\0{}", np.artist, np.track);
        if track_key != current_track {
            current_track = track_key;
            last_idx = None;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"event": "track", "artist": np.artist, "track": np.track})
                );
            } else {
                println!("{} - {}", np.artist, np.track);
            }
        }

        if let Some(ref ly) = state.lyrics {
            if ly.synced {
                let adjusted = (np.progress_ms as i64 + offset_ms).max(0) as u64;
                if let Some(idx) = lyrics::current_line_index(&ly.lines, adjusted) {
                    if last_idx != Some(idx) {
                        last_idx = Some(idx);
                        if json {
                            println!("{}", serde_json::to_string(&ly.lines[idx]).unwrap());
                        } else {
                            println!("{}", ly.lines[idx].text);
                        }
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
