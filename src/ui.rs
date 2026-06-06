//! ratatui rendering + main event loop.

use crate::app::{App, TabState};
use crate::keys;
use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
};
use std::io::Stdout;
use std::time::Duration;

pub async fn run(app: &mut App) -> Result<()> {
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let res = event_loop(&mut terminal, app).await;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;
        app.drain();
        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && key.kind == event::KeyEventKind::Press
            && let Some(action) = keys::handle(key, app)
        {
            let quit = keys::apply(action, app).await;
            if quit {
                break;
            }
        }
    }
    Ok(())
}

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(size);
    draw_tabs(f, chunks[0], app);
    draw_body(f, chunks[1], app.active());
    draw_status(f, chunks[2], app);
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let labels: Vec<Line> = app
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let suffix = if t.loading && t.items.is_empty() {
                " · scanning".to_string()
            } else if !t.items.is_empty() {
                format!(" ({})", t.items.len())
            } else {
                String::new()
            };
            Line::from(format!("{}.{}{}", i + 1, t.name, suffix))
        })
        .collect();
    let tabs = Tabs::new(labels)
        .block(Block::default().borders(Borders::ALL).title(" dynamodb "))
        .select(app.active_tab)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn draw_body(f: &mut Frame, area: Rect, tab: &TabState) {
    if let Some(err) = &tab.last_error {
        let p = Paragraph::new(format!("error: {err}\n\nPress `r` to retry."))
            .style(Style::default().fg(Color::Red));
        f.render_widget(p, area);
        return;
    }
    if tab.loading && tab.items.is_empty() {
        let p = Paragraph::new("scanning…").style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    }
    if tab.items.is_empty() {
        let p = Paragraph::new("(table empty for this scan)")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    }
    // Split: items table left (40%), focused-item detail right (60%).
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);
    let header = Row::new(vec![
        Cell::from("PRIMARY"),
        Cell::from("FIELDS"),
    ])
    .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = tab
        .items
        .iter()
        .map(|it| {
            Row::new(vec![
                Cell::from(it.primary.clone()).style(Style::default().fg(Color::Yellow)),
                Cell::from(it.secondary.clone()).style(Style::default().fg(Color::White)),
            ])
        })
        .collect();
    let widths = [Constraint::Length(28), Constraint::Min(20)];
    let title = match &tab.meta {
        Some(m) => {
            let pk = m.pk_field.clone().unwrap_or_else(|| "?".into());
            let sk = m
                .sk_field
                .as_deref()
                .map(|s| format!(" / {s}"))
                .unwrap_or_default();
            format!(" {} · pk: {pk}{sk} ", tab.name)
        }
        None => format!(" {} ", tab.name),
    };
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    let mut state = TableState::default();
    state.select(Some(tab.selected));
    f.render_stateful_widget(table, halves[0], &mut state);

    // Detail panel — show the focused item's JSON (pretty-printed).
    let focused = tab.items.get(tab.selected);
    let json = focused
        .map(|i| serde_json::to_string_pretty(&i.raw).unwrap_or_default())
        .unwrap_or_default();
    let lines: Vec<Line> = json
        .lines()
        .take((area.height as usize).saturating_sub(2))
        .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Cyan))))
        .collect();
    let detail = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" focused item "),
    );
    f.render_widget(detail, halves[1]);
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let hint = " 1-9 tab · ↑↓/jk move · o console · y yank JSON · r refresh · q quit ";
    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", app.status),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            hint,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}
