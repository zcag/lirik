use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub poll_interval_secs: u64,
    pub lyrics_offset_ms: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: "http://127.0.0.1:8888/callback".into(),
            poll_interval_secs: 5,
            lyrics_offset_ms: 0,
        }
    }
}

pub fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("lirik/config.toml")
}

pub fn load() -> Option<Config> {
    let contents = std::fs::read_to_string(path()).ok()?;
    toml::from_str(&contents).ok()
}

pub fn apply_env() {
    let Some(cfg) = load() else { return };
    if std::env::var("RSPOTIFY_CLIENT_ID").is_err() && !cfg.client_id.is_empty() {
        unsafe { std::env::set_var("RSPOTIFY_CLIENT_ID", &cfg.client_id) };
    }
    if std::env::var("RSPOTIFY_CLIENT_SECRET").is_err() && !cfg.client_secret.is_empty() {
        unsafe { std::env::set_var("RSPOTIFY_CLIENT_SECRET", &cfg.client_secret) };
    }
    if std::env::var("RSPOTIFY_REDIRECT_URI").is_err() && !cfg.redirect_uri.is_empty() {
        unsafe { std::env::set_var("RSPOTIFY_REDIRECT_URI", &cfg.redirect_uri) };
    }
}

pub fn init() {
    let p = path();
    if p.exists() {
        println!("{}", std::fs::read_to_string(&p).unwrap());
        eprintln!("# {}", p.display());
        return;
    }

    let cfg = Config {
        client_id: std::env::var("RSPOTIFY_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("RSPOTIFY_CLIENT_SECRET").unwrap_or_default(),
        ..Default::default()
    };

    let contents = toml::to_string_pretty(&cfg).unwrap();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&p, &contents).unwrap();
    println!("{contents}");
    eprintln!("# created {}", p.display());
}
