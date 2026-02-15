use rspotify::model::PlayableItem;
use rspotify::prelude::*;
use rspotify::AuthCodeSpotify;

pub struct NowPlaying {
    pub artist: String,
    pub track: String,
    pub progress_ms: u64,
    pub duration_ms: u64,
    pub is_playing: bool,
}

impl std::fmt::Display for NowPlaying {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ps = self.progress_ms / 1000;
        let ds = self.duration_ms / 1000;
        writeln!(f, "{} - {}", self.artist, self.track)?;
        write!(
            f,
            "{}:{:02}.{:03} / {}:{:02}",
            ps / 60,
            ps % 60,
            self.progress_ms % 1000,
            ds / 60,
            ds % 60,
        )?;
        if self.is_playing {
            write!(f, "  ▶")
        } else {
            write!(f, "  ⏸")
        }
    }
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
            Some(NowPlaying {
                artist,
                track: track.name,
                progress_ms,
                duration_ms: track.duration.num_milliseconds() as u64,
                is_playing: ctx.is_playing,
            })
        }
        PlayableItem::Episode(ep) => {
            let artist = ep
                .show
                .map(|s| s.name)
                .unwrap_or_else(|| "podcast".into());
            Some(NowPlaying {
                artist,
                track: ep.name,
                progress_ms,
                duration_ms: ep.duration.num_milliseconds() as u64,
                is_playing: ctx.is_playing,
            })
        }
    }
}
