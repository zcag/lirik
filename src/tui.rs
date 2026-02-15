use crate::{client, lyrics};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Padding, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::{Duration, Instant};

const ACCENT: Color = Color::Rgb(120, 200, 120);
const DIM: Color = Color::DarkGray;

struct App {
    state: crate::spotify::State,
    list_state: ListState,
    last_fetch: Instant,
    current_track: String,
    offset_ms: i64,
}

impl App {
    fn new(offset_ms: i64) -> Self {
        Self {
            state: client::fetch_state(),
            list_state: ListState::default(),
            last_fetch: Instant::now(),
            current_track: String::new(),
            offset_ms,
        }
    }

    fn progress_ms(&self) -> u64 {
        let Some(np) = client::estimate(&self.state) else { return 0 };
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
        }

        if let Some(ref ly) = self.state.lyrics {
            if ly.synced {
                self.list_state
                    .select(lyrics::current_line_index(&ly.lines, self.progress_ms()));
            }
        }
    }
}

fn progress_bar(width: u16, ratio: f64) -> Line<'static> {
    let w = width as usize;
    let filled = (w as f64 * ratio).round() as usize;
    let empty = w.saturating_sub(filled);
    Line::from(vec![
        Span::styled("\u{2501}".repeat(filled), Style::default().fg(ACCENT)),
        Span::styled("\u{2500}".repeat(empty), Style::default().fg(DIM)),
    ])
}

fn ui(f: &mut Frame, app: &mut App) {
    let outer = f.area();
    let chunks = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(1),
    ])
    .split(outer);

    let np = client::estimate(&app.state);

    // header block
    let header_area = chunks[0];
    let inner = header_area.inner(Margin::new(2, 0));

    if let Some(ref n) = np {
        let icon = if n.is_playing { "\u{25b6}" } else { "\u{23f8}" };
        let title = Line::from(vec![
            Span::styled(&n.artist, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(" - ", Style::default().fg(DIM)),
            Span::styled(&n.track, Style::default().fg(Color::White)),
            Span::raw(" "),
            Span::styled(icon, Style::default().fg(ACCENT)),
        ]);

        let ratio = if n.duration_ms > 0 {
            (n.progress_ms as f64 / n.duration_ms as f64).min(1.0)
        } else {
            0.0
        };
        let time = Line::from(vec![
            Span::styled(&n.progress, Style::default().fg(ACCENT)),
            Span::styled(" / ", Style::default().fg(DIM)),
            Span::styled(&n.duration, Style::default().fg(DIM)),
        ]);

        let bar_width = inner.width;
        let bar = progress_bar(bar_width, ratio);

        let header = Paragraph::new(vec![title, time, bar])
            .alignment(Alignment::Center);
        f.render_widget(header, inner);
    } else {
        let header = Paragraph::new(
            Span::styled("nothing playing", Style::default().fg(DIM)),
        )
        .alignment(Alignment::Center);
        f.render_widget(header, inner);
    }

    // lyrics
    let lyrics_area = chunks[1];
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
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(DIM)
                    };
                    ListItem::new(Line::from(Span::styled(&l.text, style))
                        .alignment(Alignment::Center))
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
            let msg = Paragraph::new(Span::styled("no lyrics found", Style::default().fg(DIM)))
                .block(lyrics_block)
                .alignment(Alignment::Center);
            f.render_widget(msg, lyrics_area);
        }
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
