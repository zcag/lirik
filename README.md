# lirik

Spotify lyrics in your terminal. Shows synced lyrics from [LRCLIB](https://lrclib.net) with real-time progress tracking.

## Install

```bash
cargo binstall lirik
# or
cargo install lirik
```

Or download a binary from [releases](https://github.com/zcag/lirik/releases).

## Setup

1. Create an app at [Spotify Developer Dashboard](https://developer.spotify.com/dashboard)
2. Set the redirect URI to `http://127.0.0.1:8888/callback`
3. Run `lirik config` to create `~/.config/lirik/config.toml`:

```toml
client_id = ""
client_secret = ""
redirect_uri = "http://127.0.0.1:8888/callback"
poll_interval_secs = 5
lyrics_offset_ms = 0
```

Fill in your `client_id` and `client_secret`. Alternatively, export env vars (these override config):

```sh
export RSPOTIFY_CLIENT_ID=<your-client-id>
export RSPOTIFY_CLIENT_SECRET=<your-client-secret>
export RSPOTIFY_REDIRECT_URI=http://127.0.0.1:8888/callback
```

4. Run `lirik auth login` to authenticate with Spotify (opens browser)

## Usage

```
lirik                   interactive TUI with synced lyrics (default)
lirik -j                full JSON output
lirik -p                print all lyrics
lirik -pc               print from current line to end
lirik -pr               print all lyrics in reverse
lirik -pcr              print from current line to end, reversed
lirik -w                stream lyrics line by line as they play
lirik -wj               stream as JSON (ndjson)
```

### Options

| Short | Long | Description |
|-------|------|-------------|
| | *(default)* | Interactive TUI with synced lyrics |
| `-j` | `--json` | Full JSON (track, progress, current lyric, all lyrics) |
| `-p` | `--plain` | Print lyrics to stdout |
| `-c` | `--current` | With `-p`: start from current line |
| `-r` | `--reverse` | With `-p`: reverse output order |
| `-w` | `--watch` | Stream lyrics line by line |
| `-o` | `--offset <ms>` | Shift lyrics timing (positive = earlier) |

Short flags combine: `-pcr` = `--plain --current --reverse`

### Commands

| Command | Description |
|---------|-------------|
| `auth` | Show auth & credential status |
| `auth login` | Open browser to authenticate with Spotify |
| `config` | Create/show config file |
| `restart` | Kill and restart daemon in foreground |
| `stop` | Kill daemon |

## Architecture

lirik runs a background daemon that polls Spotify every few seconds and caches the current track + lyrics. All client commands connect to the daemon over a unix socket (`/tmp/lirik.sock`) for instant responses.

The daemon starts automatically on first use. No manual setup needed.

**Daemon** polls Spotify for playback state, fetches lyrics from LRCLIB on track change, and serves everything over the socket.

**Client** connects to the daemon, reads cached state, and estimates progress client-side from the baseline + elapsed wall time.

### JSON output

`lirik -j` returns:

```json
{
  "artist": "Artist Name",
  "track": "Track Name",
  "progress_ms": 123456,
  "progress": "2:03",
  "duration_ms": 234567,
  "duration": "3:54",
  "is_playing": true,
  "lyric": "current lyric line",
  "lyrics": {
    "synced": true,
    "lines": [
      {"time_ms": 12340, "text": "first line"},
      {"time_ms": 15670, "text": "second line"}
    ]
  }
}
```

### Watch mode

`lirik -w` streams one line at a time:

```
Artist - Track
first lyric line
second lyric line
                        <- empty line on pause
Artist - Track
next lyric line
```

`lirik -wj` streams ndjson:

```json
{"event":"track","artist":"Artist","track":"Track"}
{"time_ms":12340,"text":"first line"}
{"time_ms":15670,"text":"second line"}
```

### Config

`~/.config/lirik/config.toml`:

| Key | Default | Description |
|-----|---------|-------------|
| `client_id` | `""` | Spotify app client ID |
| `client_secret` | `""` | Spotify app client secret |
| `redirect_uri` | `http://127.0.0.1:8888/callback` | OAuth redirect URI |
| `poll_interval_secs` | `5` | How often the daemon polls Spotify (seconds) |
| `lyrics_offset_ms` | `0` | Default lyrics timing offset (ms, positive = earlier) |

Env vars (`RSPOTIFY_CLIENT_ID`, `RSPOTIFY_CLIENT_SECRET`, `RSPOTIFY_REDIRECT_URI`) override config values.

## License

MIT
