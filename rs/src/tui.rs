use anyhow::Context;
use crossterm::cursor::Hide;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use futures::StreamExt;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());

    let peer_items: Vec<ListItem> = if app.peers.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  Waiting for others to join…",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        app.peers
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
                ListItem::new(Line::from(vec![label, content]))
            })
            .collect()
    };

    let title = format!(
        " moq-keycast :: {} | {} peer{} ",
        app.room,
        app.peers.len(),
        if app.peers.len() == 1 { "" } else { "s" }
    );

    frame.render_widget(
        List::new(peer_items).block(
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

    // '█' is a fake cursor — the real cursor stays hidden (ratatui hides it when
    // set_cursor_position is not called).
    let input_line = format!("  {}: {}█", app.username, app.input);
    frame.render_widget(
        Paragraph::new(input_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        ),
        chunks[1],
    );
}
