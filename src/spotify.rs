use rspotify::model::PlayableItem;
use rspotify::prelude::*;
use rspotify::AuthCodeSpotify;

pub fn fmt_time(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct NowPlaying {
    pub artist: String,
    pub track: String,
    pub progress_ms: u64,
    pub progress: String,
    pub duration_ms: u64,
    pub duration: String,
    pub is_playing: bool,
}

impl std::fmt::Display for NowPlaying {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} - {}", self.artist, self.track)?;
        write!(f, "{} / {}", self.progress, self.duration)?;
        if self.is_playing {
            write!(f, "  ▶")
        } else {
            write!(f, "  ⏸")
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct State {
    pub now_playing: Option<NowPlaying>,
    pub fetched_at_ms: u64,
    pub lyrics: Option<crate::lyrics::Lyrics>,
}

pub async fn now_playing(spotify: &AuthCodeSpotify) -> Option<NowPlaying> {
    let ctx = spotify.current_playing(None, None::<Vec<_>>).await.ok()??;
    let progress_ms = ctx.progress.map(|p| p.num_milliseconds() as u64).unwrap_or(0);

    match ctx.item? {
        PlayableItem::Track(track) => {
            let artist = track
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let duration_ms = track.duration.num_milliseconds() as u64;
            Some(NowPlaying {
                artist,
                track: track.name,
                progress: fmt_time(progress_ms),
                progress_ms,
                duration: fmt_time(duration_ms),
                duration_ms,
                is_playing: ctx.is_playing,
            })
        }
        PlayableItem::Episode(ep) => {
            let artist = ep.show.name;
            let duration_ms = ep.duration.num_milliseconds() as u64;
            Some(NowPlaying {
                artist,
                track: ep.name,
                progress: fmt_time(progress_ms),
                progress_ms,
                duration: fmt_time(duration_ms),
                duration_ms,
                is_playing: ctx.is_playing,
            })
        }
    }
}
