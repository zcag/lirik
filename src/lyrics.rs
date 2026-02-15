use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Lyrics {
    pub synced: bool,
    pub lines: Vec<LyricLine>,
}

#[derive(Deserialize)]
struct LrcLibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

pub async fn fetch(artist: &str, track: &str, duration_ms: u64) -> Option<Lyrics> {
    let resp = reqwest::Client::new()
        .get("https://lrclib.net/api/get")
        .header("User-Agent", "lirik/0.1.0")
        .query(&[
            ("artist_name", artist),
            ("track_name", track),
            ("duration", &(duration_ms / 1000).to_string()),
        ])
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let data: LrcLibResponse = resp.json().await.ok()?;

    if let Some(ref synced) = data.synced_lyrics {
        let lines = parse_lrc(synced);
        if !lines.is_empty() {
            return Some(Lyrics { synced: true, lines });
        }
    }

    let plain = data.plain_lyrics?;
    let lines = plain
        .lines()
        .map(|l| LyricLine { time_ms: 0, text: l.to_string() })
        .collect();
    Some(Lyrics { synced: false, lines })
}

fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    lrc.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix('[')?;
            let (time_str, text) = rest.split_once(']')?;
            let (min_str, rest) = time_str.split_once(':')?;
            let (sec_str, frac_str) = rest.split_once('.')?;
            let min: u64 = min_str.parse().ok()?;
            let sec: u64 = sec_str.parse().ok()?;
            let frac: u64 = frac_str.parse().ok()?;
            let frac_ms = match frac_str.len() {
                1 => frac * 100,
                2 => frac * 10,
                3 => frac,
                _ => frac,
            };
            Some(LyricLine {
                time_ms: min * 60000 + sec * 1000 + frac_ms,
                text: text.trim().to_string(),
            })
        })
        .collect()
}

pub fn current_line_index(lines: &[LyricLine], progress_ms: u64) -> Option<usize> {
    if lines.is_empty() || lines[0].time_ms > progress_ms {
        return None;
    }
    let mut idx = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.time_ms <= progress_ms {
            idx = i;
        } else {
            break;
        }
    }
    Some(idx)
}
