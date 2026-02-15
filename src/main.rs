mod auth;
mod client;
mod config;
mod daemon;
mod lyrics;
mod spotify;
mod tui;
mod watch;
mod web;

use rspotify::{scopes, AuthCodeSpotify, Config, Credentials, OAuth};
use std::env;

fn make_client() -> AuthCodeSpotify {
    config::apply_env();

    let creds = Credentials::from_env()
        .expect("set RSPOTIFY_CLIENT_ID and RSPOTIFY_CLIENT_SECRET (or run `lirik config`)");
    let oauth = OAuth::from_env(scopes!(
        "user-read-currently-playing",
        "user-read-playback-state"
    ))
    .expect("set RSPOTIFY_REDIRECT_URI=http://127.0.0.1:8888/callback");

    let config = Config {
        token_cached: true,
        token_refreshing: true,
        ..Default::default()
    };

    AuthCodeSpotify::with_config(creds, oauth, config)
}

fn has(args: &[String], short: char, long: &str) -> bool {
    args.iter().any(|a| {
        a == long
            || (a.starts_with('-')
                && !a.starts_with("--")
                && a[1..].contains(short))
    })
}

fn parse_web_port(args: &[String]) -> u16 {
    args.iter()
        .position(|a| a == "--web")
        .map(|i| {
            args.get(i + 1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(3000)
        })
        .unwrap_or_else(|| config::load().map(|c| c.web_port).unwrap_or(0))
}

fn parse_offset(args: &[String]) -> i64 {
    args.iter()
        .position(|a| a == "--offset" || a == "-o")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| config::load().map(|c| c.lyrics_offset_ms).unwrap_or(0))
}

fn run_async(f: impl std::future::Future<Output = ()>) {
    tokio::runtime::Runtime::new().unwrap().block_on(f);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("--help" | "-h") => {
            print!(
                "\
lirik - spotify lyrics in your terminal

usage: lirik [options]
       lirik <command>

options:
  (default)             interactive TUI with synced lyrics
  -j, --json            full JSON (track, progress, current lyric, all lyrics)
  -p, --plain           print all lyrics to stdout
  -c, --current         with -p: from current line to end
  -r, --reverse         with -p: reverse output order
  -w, --watch           stream lyrics line by line as they play
  -o, --offset <ms>     shift lyrics timing (positive = earlier)
  --web [port]          enable web UI (default port: 3000)

  flags combine: -pcr = --plain --current --reverse

commands:
  auth                  show auth & credential status
  auth login            open browser to authenticate with Spotify
  config                create/show config (~/.config/lirik/config.toml)
  restart               kill and restart daemon in foreground
  stop                  kill daemon
  --daemon              start background daemon (auto-started normally)
  -h, --help            show this help
"
            );
        }
        Some("--daemon") => run_async(async {
            let spotify = make_client();
            auth::authenticate(&spotify).await;
            let poll_secs = config::load().map(|c| c.poll_interval_secs).unwrap_or(5);
            let web_port = parse_web_port(&args);
            daemon::run(spotify, poll_secs, web_port).await;
        }),
        Some("auth") => run_async(async {
            let spotify = make_client();
            match args.get(2).map(|s| s.as_str()) {
                Some("login") => auth::login(&spotify).await,
                _ => auth::status(&spotify).await,
            }
        }),
        Some("config") => config::init(),
        Some("restart") => {
            daemon::kill();
            client::spawn_daemon();
            eprintln!("daemon restarted");
        }
        Some("stop") => daemon::kill(),
        _ => {
            let json = has(&args, 'j', "--json");
            let watch = has(&args, 'w', "--watch");
            let plain = has(&args, 'p', "--plain");
            let current = has(&args, 'c', "--current");
            let reverse = has(&args, 'r', "--reverse");
            let offset = parse_offset(&args);

            if watch {
                watch::run(json, offset);
            } else if plain {
                client::plain(current, reverse, offset);
            } else if json {
                client::json(offset);
            } else {
                tui::run(offset);
            }
        }
    }
}
