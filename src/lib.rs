#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc)]

pub mod kube;

use self::kube::EventV1;
use chrono::{DateTime, Utc};
use crossterm::{
    self,
    event::{Event, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Row, Table, TableState, Wrap},
    Frame, Terminal,
};
use std::{
    collections::BTreeMap,
    io::{stdout, Stdout},
};

const NUM_TABLE_ROWS: usize = 12;
type EventsMap = BTreeMap<DateTime<Utc>, (EventV1, [String; 3])>;

pub struct App {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    events: EventsMap,
    table_state: TableState,
    events_position: usize,
    scroll_position: u16,
    infobox: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    #[must_use]
    pub fn new() -> Self {
        Self {
            terminal: Terminal::new(CrosstermBackend::new(stdout()))
                .expect("failed to get stdout for terminal output"),
            events: BTreeMap::new(),
            table_state: TableState::new(),
            events_position: 0,
            scroll_position: 0,
            infobox: None,
        }
    }

    pub fn setup(&mut self) {
        stdout()
            .execute(EnterAlternateScreen)
            .expect("failed to enter alternate screen");
        enable_raw_mode().expect("failed to enter raw mode");
        self.terminal.clear().expect("failed to clear terminal");
    }

    pub fn tear_down(&mut self) {
        stdout()
            .execute(LeaveAlternateScreen)
            .expect("failed to leave alternate screen");
        disable_raw_mode().expect("failed to disable raw mode");
    }

    pub fn handle_kube_event(&mut self, event: EventV1) {
        let row = [
            event.request_received_timestamp.to_string(),
            event.verb.clone(),
            event
                .request_uri
                .split('?')
                .next()
                .expect("iterator is valid")
                .to_string(),
        ];
        self.events
            .insert(event.request_received_timestamp, (event, row));

        if self.table_state.selected().is_none() {
            self.table_state.select(Some(0));
        }
    }

    pub fn handle_terminal_event(&mut self, event: std::io::Result<Event>) -> Option<()> {
        match event {
            Ok(event) => {
                if let Event::Key(KeyEvent { code, .. }) = event {
                    match code {
                        KeyCode::Esc | KeyCode::Char('q') => return Some(()),
                        KeyCode::Up => self.previous(1),
                        KeyCode::PageUp => self.previous(5),
                        KeyCode::Down => self.next(1),
                        KeyCode::PageDown => self.next(5),
                        KeyCode::Char('k') => self.scroll_up(),
                        KeyCode::Char('j') => self.scroll_down(),
                        _ => {}
                    };
                }

                None
            }
            Err(err) => {
                self.set_error(err.into());
                None
            }
        }
    }

    pub fn draw(&mut self) {
        self.draw_events();
    }

    pub fn draw_events(&mut self) {
        self.terminal
            .draw(|frame| {
                // outer frame
                let frame_area = frame.area();
                let frame_block = Block::bordered()
                    .title("Kubernetes Audit Log Explorer (KALE)")
                    .border_type(BorderType::Rounded);
                let inner = frame_block.inner(frame_area);
                frame.render_widget(frame_block, frame_area);

                // if index is None, we have nothing to draw.
                let Some(offset) = self.table_state.selected() else {
                    return;
                };
                // factor in the offset from the events window when getting the index
                let index = self.events_position + offset;
                // check that index is in range
                if !(0..self.events.len()).contains(&index) {
                    self.infobox = Some(format!(
                        "attempted to access event #{index} which we don't have",
                    ));
                    return;
                }
                let (event, _row) = self.events.values().nth(index).expect("we checked");

                // overall layout
                let [table_area, info_area, req_res_area, infobox_area] = Layout::vertical([
                    Constraint::Length(
                        u16::try_from(NUM_TABLE_ROWS).expect("we know the value is valid") + 1,
                    ),
                    Constraint::Length(7 + 1),
                    Constraint::Fill(1),
                    Constraint::Length(1 + 1),
                ])
                .areas(inner);
                let [left, right] =
                    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .areas(req_res_area);
                // left & right blocks
                let left_block = Block::new()
                    .title("Request")
                    .borders(Borders::TOP | Borders::RIGHT)
                    .border_type(BorderType::Rounded)
                    .padding(Padding::left(1));
                let left_inner = left_block.inner(left);
                let right_block = Block::new()
                    .title("Response")
                    .borders(Borders::TOP)
                    .border_type(BorderType::Rounded)
                    .padding(Padding::left(1));
                let right_inner = right_block.inner(right);
                frame.render_widget(left_block, left);
                frame.render_widget(right_block, right);

                draw_table(
                    frame,
                    table_area,
                    &self.events,
                    self.events_position,
                    &mut self.table_state,
                );
                draw_info(frame, info_area, event);
                draw_request_info(frame, left_inner, event, self.scroll_position);
                draw_response_info(frame, right_inner, event, self.scroll_position);
                draw_infobox(frame, infobox_area, self.infobox.as_deref().unwrap_or(""));
            })
            .expect("failed to draw frame");
    }

    fn previous(&mut self, by: usize) {
        if let Some(index) = self.table_state.selected() {
            if index == 0 {
                self.events_position = self.events_position.saturating_sub(by);
            } else {
                self.table_state.select(Some(index.saturating_sub(by)));
            }
        }

        self.scroll_position = 0;
    }

    fn next(&mut self, by: usize) {
        let num_events = self.events.len();

        // this is a bit jank, but this is ensuring that we first saturate event_pane_offset (self.table_state.select())
        // before we move the self.events_position, which is the overall offset within the "list" of Events.
        for _ in 0..by {
            if let Some(event_pane_offset) = self.table_state.selected() {
                if event_pane_offset == NUM_TABLE_ROWS - 1 {
                    // if at the bottom of the event pane, move the event position
                    self.events_position =
                        (self.events_position + 1).min(num_events - NUM_TABLE_ROWS);
                } else {
                    // if not at the bottom of the event pane, move the event pane offset.
                    self.table_state
                        .select(Some((event_pane_offset + 1).min(num_events - 1)));
                }
            }
        }

        self.scroll_position = 0;
    }

    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(3);
    }

    fn scroll_down(&mut self) {
        self.scroll_position += 3;
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn set_error(&mut self, error: anyhow::Error) {
        self.infobox = Some(error.to_string());
    }
}

fn draw_table(
    frame: &mut Frame,
    area: Rect,
    events: &EventsMap,
    events_position: usize,
    table_state: &mut TableState,
) {
    let table = Table::default()
        .white()
        .on_black()
        .rows(
            events
                .values()
                .skip(events_position)
                .take(NUM_TABLE_ROWS)
                .map(|(_event, cells)| Row::new(cells.clone())),
        )
        .widths([
            Constraint::Length(30),
            Constraint::Length(6),
            Constraint::Fill(1),
        ])
        .column_spacing(1)
        .header(Row::new(["timestamp", "verb", "request uri"]).underlined())
        .row_highlight_style(Style::new().black().on_gray());
    frame.render_stateful_widget(table, area, table_state);
}

fn draw_info(frame: &mut Frame, area: Rect, event: &EventV1) {
    let info_block = Block::new()
        .title("Request Info")
        .borders(Borders::TOP)
        .border_type(BorderType::Rounded)
        .padding(Padding::left(1));
    let info_inner = info_block.inner(area);
    frame.render_widget(info_block, area);
    let info_text = format!(
        "Request URI:       {}
Audit ID:          {}
Object Ref:        {}
User:              {}
Impersonated User: {}
User Agent:        {}
Source IPs:        {}
",
        event.request_uri,
        event.audit_id,
        event
            .object_ref
            .as_ref()
            .map_or_else(|| "N/A".to_string(), ToString::to_string),
        event.user.username,
        event
            .impersonated_user
            .as_ref()
            .map_or("N/A", |imp_user| imp_user.username.as_str()),
        event.user_agent.as_deref().unwrap_or("N/A"),
        event
            .source_ips
            .as_ref()
            .map(|ips| ips.iter().map(ToString::to_string).collect::<Vec<_>>())
            .map_or_else(|| "N/A".to_string(), |ips| ips.join(", ")),
    );
    frame.render_widget(Paragraph::new(info_text), info_inner);
}

fn draw_request_info(frame: &mut Frame, area: Rect, event: &EventV1, scroll_position: u16) {
    let request_text = event
        .request_object
        .as_ref()
        .map(|req| format!("{req:#}"))
        .unwrap_or_default()
        .to_string();
    frame.render_widget(
        Paragraph::new(request_text)
            .wrap(Wrap { trim: false })
            .scroll((scroll_position, 0))
            .white()
            .on_black(),
        area,
    );
}

fn draw_response_info(frame: &mut Frame, area: Rect, event: &EventV1, scroll_position: u16) {
    let response_text = event
        .response_object
        .as_ref()
        .map(|res| format!("{res:#}"))
        .unwrap_or_default()
        .to_string();
    frame.render_widget(
        Paragraph::new(response_text)
            .wrap(Wrap { trim: false })
            .scroll((scroll_position, 0))
            .white()
            .on_black(),
        area,
    );
}

fn draw_infobox(frame: &mut Frame, area: Rect, text: &str) {
    let infobox_block = Block::new()
        .title("Info")
        .borders(Borders::TOP)
        .border_type(BorderType::Rounded)
        .padding(Padding::left(1));
    let infobox_inner = infobox_block.inner(area);
    frame.render_widget(infobox_block, area);
    frame.render_widget(Paragraph::new(text), infobox_inner);
}
