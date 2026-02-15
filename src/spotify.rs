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
    pub album: String,
    pub album_art: Option<String>,
    pub popularity: u32,
    pub explicit: bool,
    pub spotify_url: Option<String>,
    pub progress_ms: u64,
    pub progress: String,
    pub duration_ms: u64,
    pub duration: String,
    pub is_playing: bool,
    pub device: Option<DeviceInfo>,
    pub shuffle: bool,
    pub repeat: String,
    pub context: Option<PlayContext>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub device_type: String,
    pub volume: Option<u32>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayContext {
    pub context_type: String,
    pub uri: String,
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
    let ctx = spotify
        .current_playback(None, None::<Vec<_>>)
        .await
        .ok()??;
    let progress_ms = ctx
        .progress
        .map(|p| p.num_milliseconds() as u64)
        .unwrap_or(0);

    let device = Some(DeviceInfo {
        name: ctx.device.name,
        device_type: format!("{:?}", ctx.device._type),
        volume: ctx.device.volume_percent,
    });
    let shuffle = ctx.shuffle_state;
    let repeat = format!("{:?}", ctx.repeat_state).to_lowercase();
    let play_context = ctx.context.map(|c| PlayContext {
        context_type: format!("{:?}", c._type).to_lowercase(),
        uri: c.uri,
    });

    match ctx.item? {
        PlayableItem::Track(track) => {
            let artist = track
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let duration_ms = track.duration.num_milliseconds() as u64;
            let album_art = track
                .album
                .images
                .first()
                .map(|img| img.url.clone());
            let spotify_url = track
                .external_urls
                .get("spotify")
                .cloned();
            Some(NowPlaying {
                artist,
                track: track.name,
                album: track.album.name,
                album_art,
                popularity: track.popularity,
                explicit: track.explicit,
                spotify_url,
                progress: fmt_time(progress_ms),
                progress_ms,
                duration: fmt_time(duration_ms),
                duration_ms,
                is_playing: ctx.is_playing,
                device,
                shuffle,
                repeat,
                context: play_context,
            })
        }
        PlayableItem::Episode(ep) => {
            let duration_ms = ep.duration.num_milliseconds() as u64;
            let album_art = ep.images.first().map(|img| img.url.clone());
            let spotify_url = ep.external_urls.get("spotify").cloned();
            Some(NowPlaying {
                artist: ep.show.name,
                track: ep.name,
                album: String::new(),
                album_art,
                popularity: 0,
                explicit: ep.explicit,
                spotify_url,
                progress: fmt_time(progress_ms),
                progress_ms,
                duration: fmt_time(duration_ms),
                duration_ms,
                is_playing: ctx.is_playing,
                device,
                shuffle,
                repeat,
                context: play_context,
            })
        }
    }
}
