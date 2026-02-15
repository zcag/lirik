use crate::{client, lyrics};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Padding, Paragraph},
    Frame, Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};
use std::io;
use std::time::{Duration, Instant};

const ACCENT: Color = Color::Green;
const DIM: Color = Color::DarkGray;

struct App {
    state: crate::spotify::State,
    list_state: ListState,
    last_fetch: Instant,
    current_track: String,
    offset_ms: i64,
    picker: Option<Picker>,
    art: Option<StatefulProtocol>,
    art_url: String,
}

impl App {
    fn new(offset_ms: i64) -> Self {
        let picker = Picker::from_query_stdio().ok();
        Self {
            state: client::fetch_state(),
            list_state: ListState::default(),
            last_fetch: Instant::now(),
            current_track: String::new(),
            offset_ms,
            picker,
            art: None,
            art_url: String::new(),
        }
    }

    fn progress_ms(&self) -> u64 {
        let Some(np) = client::estimate(&self.state) else {
            return 0;
        };
        (np.progress_ms as i64 + self.offset_ms).max(0) as u64
    }

    fn update(&mut self) {
        if self.last_fetch.elapsed() > Duration::from_secs(2) {
            self.state = client::fetch_state();
            self.last_fetch = Instant::now();
        }

        let track_key = self
            .state
            .now_playing
            .as_ref()
            .map(|n| format!("{}\0{}", n.artist, n.track))
            .unwrap_or_default();

        if track_key != self.current_track {
            self.current_track = track_key;
            self.list_state = ListState::default();
            self.update_art();
        }

        if let Some(ly) = &self.state.lyrics {
            if ly.synced {
                self.list_state
                    .select(lyrics::current_line_index(&ly.lines, self.progress_ms()));
            }
        }
    }

    fn update_art(&mut self) {
        let url = self
            .state
            .now_playing
            .as_ref()
            .and_then(|n| n.album_art.as_deref())
            .unwrap_or("");

        if url == self.art_url {
            return;
        }
        self.art_url = url.to_string();
        self.art = None;

        let Some(picker) = &self.picker else { return };
        if url.is_empty() {
            return;
        }

        if let Ok(bytes) = reqwest::blocking::get(url).and_then(|r| r.bytes()) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                self.art = Some(picker.new_resize_protocol(img));
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let outer = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(outer);

    let np = client::estimate(&app.state);

    // --- header ---
    let header_area = chunks[1];
    render_header(f, header_area, &np);

    // --- separator ---
    let sep_area = chunks[2];
    let sep = Paragraph::new(Line::from(Span::styled(
        "\u{2500}".repeat(sep_area.width as usize),
        Style::default().fg(DIM),
    )));
    f.render_widget(sep, sep_area);

    // --- lyrics ---
    let lyrics_area = chunks[3];
    let lyrics_block = Block::default().padding(Padding::horizontal(2));

    match &app.state.lyrics {
        Some(ly) if !ly.lines.is_empty() => {
            let selected = app.list_state.selected();
            let items: Vec<ListItem> = ly
                .lines
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let style = if selected == Some(i) {
                        Style::default()
                            .fg(ACCENT)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        let d = selected.map(|s| i.abs_diff(s)).unwrap_or(999);
                        if d <= 2 {
                            Style::default().fg(Color::Gray)
                        } else {
                            Style::default().fg(DIM)
                        }
                    };
                    let text = if l.text.is_empty() { " " } else { &l.text };
                    ListItem::new(
                        Line::from(Span::styled(text, style)).alignment(Alignment::Center),
                    )
                })
                .collect();

            if let Some(sel) = selected {
                let inner_h = lyrics_area.height.saturating_sub(2) as usize;
                *app.list_state.offset_mut() = sel.saturating_sub(inner_h / 2);
            }

            let list = List::new(items).block(lyrics_block);
            f.render_stateful_widget(list, lyrics_area, &mut app.list_state);
        }
        _ => {
            let msg =
                Paragraph::new(Span::styled("no lyrics found", Style::default().fg(DIM)))
                    .block(lyrics_block)
                    .alignment(Alignment::Center);
            f.render_widget(msg, lyrics_area);
        }
    }

    // --- album art (bottom-left, floating over lyrics) ---
    if let Some(proto) = &mut app.art {
        let art_h = 5u16;
        let art_w = 10u16;
        let art_area = Rect {
            x: 1,
            y: outer.height.saturating_sub(art_h + 1),
            width: art_w,
            height: art_h,
        };
        f.render_stateful_widget(StatefulImage::default(), art_area, proto);
    }
}

fn render_header(
    f: &mut Frame,
    area: Rect,
    np: &Option<crate::spotify::NowPlaying>,
) {
    let text_area = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    if let Some(n) = np {
        let icon = if n.is_playing { " \u{25b6}" } else { " \u{23f8}" };

        // line 1: track name
        let title = Line::from(vec![
            Span::styled(
                &n.track,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(icon, Style::default().fg(ACCENT)),
        ]);

        // line 2: artist - album
        let mut sub_spans = vec![Span::styled(&n.artist, Style::default().fg(Color::Gray))];
        if !n.album.is_empty() {
            sub_spans.push(Span::styled(" \u{2014} ", Style::default().fg(DIM)));
            sub_spans.push(Span::styled(&n.album, Style::default().fg(DIM)));
        }
        let sub = Line::from(sub_spans);

        // line 3: elapsed ━━━━━━━━────────── total
        let ratio = if n.duration_ms > 0 {
            (n.progress_ms as f64 / n.duration_ms as f64).min(1.0)
        } else {
            0.0
        };

        let time_l = format!("{} ", n.progress);
        let time_r = format!(" {}", n.duration);
        let bar_w = text_area
            .width
            .saturating_sub(time_l.len() as u16 + time_r.len() as u16)
            as usize;
        let filled = (bar_w as f64 * ratio).round() as usize;
        let empty = bar_w.saturating_sub(filled);

        let progress_line = Line::from(vec![
            Span::styled(&time_l, Style::default().fg(ACCENT)),
            Span::styled("\u{2501}".repeat(filled), Style::default().fg(ACCENT)),
            Span::styled("\u{2500}".repeat(empty), Style::default().fg(DIM)),
            Span::styled(&time_r, Style::default().fg(DIM)),
        ]);

        let header = Paragraph::new(vec![title, sub, Line::raw(""), progress_line])
            .alignment(Alignment::Center);
        f.render_widget(header, text_area);
    } else {
        let header = Paragraph::new(Span::styled(
            "nothing playing",
            Style::default().fg(DIM),
        ))
        .alignment(Alignment::Center);
        f.render_widget(header, text_area);
    }
}

pub fn run(offset_ms: i64) {
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(offset_ms);
    app.update();

    loop {
        terminal.draw(|f| ui(f, &mut app)).unwrap();

        if event::poll(Duration::from_millis(100)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
            }
        }

        app.update();
    }

    disable_raw_mode().unwrap();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).unwrap();
}
