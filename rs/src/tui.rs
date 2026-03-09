use anyhow::Context;
use crossterm::cursor::Hide;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use futures::StreamExt;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    DefaultTerminal, Frame,
};
use std::collections::BTreeMap;
use std::io::stdout;
use tokio::sync::mpsc;

pub enum PeerEvent {
    Joined(String),
    Update(String, String),
    Offline(String),
}

struct App {
    room: String,
    username: String,
    /// Remote peers' live text, keyed by username. BTreeMap keeps display order stable.
    peers: BTreeMap<String, String>,
    input: String,
}

impl App {
    fn new(room: String, username: String) -> Self {
        Self {
            room,
            username,
            peers: BTreeMap::new(),
            input: String::new(),
        }
    }
}

pub async fn run(
    room: String,
    username: String,
    typing_tx: mpsc::UnboundedSender<String>,
    mut peer_rx: mpsc::UnboundedReceiver<PeerEvent>,
) -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    // Hide cursor before the first frame; ratatui hides it after each draw but
    // not before the very first one.
    let _ = execute!(stdout(), Hide);
    let result = event_loop(&mut terminal, room, username, typing_tx, &mut peer_rx).await;
    ratatui::restore();
    result
}

async fn event_loop(
    terminal: &mut DefaultTerminal,
    room: String,
    username: String,
    typing_tx: mpsc::UnboundedSender<String>,
    peer_rx: &mut mpsc::UnboundedReceiver<PeerEvent>,
) -> anyhow::Result<()> {
    let mut app = App::new(room, username);
    let mut events = EventStream::new();

    loop {
        terminal.draw(|f| render(f, &app)).context("render")?;

        tokio::select! {
            Some(Ok(event)) = events.next() => {
                let Event::Key(key) = event else { continue };
                if key.kind != KeyEventKind::Press { continue; }

                match key.code {
                    KeyCode::Esc => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char(c) => {
                        app.input.push(c);
                        let _ = typing_tx.send(app.input.clone());
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                        let _ = typing_tx.send(app.input.clone());
                    }
                    // Enter clears input and sends an empty update to reset others' view.
                    KeyCode::Enter => {
                        app.input.clear();
                        let _ = typing_tx.send(String::new());
                    }
                    _ => {}
                }
            }

            Some(event) = peer_rx.recv() => {
                match event {
                    PeerEvent::Joined(username) => { app.peers.entry(username).or_default(); }
                    PeerEvent::Update(username, text) => { app.peers.insert(username, text); }
                    PeerEvent::Offline(username) => { app.peers.remove(&username); }
                }
            }
        }
    }

    Ok(())
}

fn render(frame: &mut Frame, app: &App) {
    // Pre-compute how many rows the input box needs so the layout can reserve them.
    // Content width = terminal width minus 2 border columns.
    let content_width = frame.area().width.saturating_sub(2).max(1);
    // '█' is a fake cursor — the real cursor stays hidden (ratatui hides it when
    // set_cursor_position is not called).
    let input_line = format!("  {}: {}█", app.username, app.input);
    // Use char count (not byte count) for display-width accuracy, then add 1 row
    // as a buffer because Paragraph wraps at word boundaries, not character
    // boundaries, so the real row count can exceed the simple division.
    let wrapped_input_rows = (input_line.chars().count() as u16)
        .div_ceil(content_width)
        .saturating_add(1)
        .min(8); // cap so the input box never consumes the whole screen
    let input_height = wrapped_input_rows + 2; // +2 for top/bottom borders

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(input_height)])
        .split(frame.area());

    let peers_text: Text = if app.peers.is_empty() {
        Text::from(Line::from(Span::styled(
            "  Waiting for others to join…",
            Style::default().fg(Color::DarkGray),
        )))
    } else {
        let lines: Vec<Line> = app
            .peers
            .iter()
            .map(|(name, text)| {
                let label = Span::styled(
                    format!("  {name}: "),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
                let content = if text.is_empty() {
                    Span::styled("…", Style::default().fg(Color::DarkGray))
                } else {
                    Span::raw(text.clone())
                };
                Line::from(vec![label, content])
            })
            .collect();
        Text::from(lines)
    };

    let title = format!(
        " moq-keycast :: {} | {} peer{} ",
        app.room,
        app.peers.len(),
        if app.peers.len() == 1 { "" } else { "s" }
    );

    frame.render_widget(
        Paragraph::new(peers_text).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(title)
                .title_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(input_line).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        ),
        chunks[1],
    );
}
