use crate::spotify::State;
use rspotify::AuthCodeSpotify;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

const PAGE: &str = include_str!("web.html");

pub async fn serve(port: u16, state: Arc<RwLock<State>>, client: Arc<AuthCodeSpotify>) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{port}")).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("web ui: failed to bind port {port}: {e}");
            return;
        }
    };

    eprintln!("web ui at http://localhost:{port}");

    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };
        let state = state.clone();
        let client = client.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            let req = String::from_utf8_lossy(&buf[..n]);
            let first_line = req.lines().next().unwrap_or("");
            let method = first_line.split_whitespace().next().unwrap_or("");
            let path = first_line.split_whitespace().nth(1).unwrap_or("/");

            let (status, ctype, body) = if path == "/api/state" {
                let s = state.read().await;
                ("200 OK", "application/json", serde_json::to_string(&*s).unwrap())
            } else if path == "/api/cmd" && method == "POST" {
                // extract JSON body after \r\n\r\n
                let req_str = req.to_string();
                let body_str = req_str
                    .split("\r\n\r\n")
                    .nth(1)
                    .unwrap_or("")
                    .to_string();
                let resp = match serde_json::from_str::<serde_json::Value>(&body_str) {
                    Ok(v) => {
                        let cmd = v["cmd"].as_str().unwrap_or("");
                        let arg = v["arg"].as_str();
                        match crate::daemon::execute_cmd(&client, &state, cmd, arg).await {
                            Ok(()) => r#"{"ok":true}"#.to_string(),
                            Err(e) => {
                                let e = e.replace('"', r#"\""#);
                                format!(r#"{{"error":"{e}"}}"#)
                            }
                        }
                    }
                    Err(e) => format!(r#"{{"error":"bad json: {e}"}}"#),
                };
                ("200 OK", "application/json", resp)
            } else {
                ("200 OK", "text/html", PAGE.to_string())
            };

            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes()).await;
        });
    }
}
