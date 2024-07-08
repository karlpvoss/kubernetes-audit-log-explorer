pub mod kube;

use self::kube::EventV1;
use crossterm::{
    self,
    event::{Event, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Row, Table, TableState, Wrap},
    Terminal,
};
use std::io::{stdout, Stdout};

pub struct App {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    events: Vec<EventV1>,
    table_rows: Vec<[String; 3]>,
    table_state: TableState,
    scroll_position: u16,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            terminal: Terminal::new(CrosstermBackend::new(stdout()))
                .expect("failed to get stdout for terminal output"),
            events: Vec::new(),
            table_rows: Vec::new(),
            table_state: TableState::new(),
            scroll_position: 0,
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
        self.events.push(event.clone());
        let base_uri = event
            .request_uri
            .split('?')
            .next()
            .expect("iterator is valid")
            .to_string();
        self.table_rows.push([
            event.request_received_timestamp.to_string(),
            event.verb,
            base_uri,
        ]);
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
                        KeyCode::Up => self.previous(),
                        KeyCode::Down => self.next(),
                        KeyCode::PageUp => self.scroll_up(),
                        KeyCode::PageDown => self.scroll_down(),
                        _ => {}
                    };
                }

                None
            }
            Err(err) => {
                eprintln!("{}", err);
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
                let i = self.table_state.selected();
                let event = i.map(|i| &self.events[i]);

                // frame
                let frame_area = frame.size();
                let frame_block = Block::new()
                    .title("Kubernetes Audit Log Explorer (KALE)")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded);
                let frame_inner = frame_block.inner(frame_area);
                frame.render_widget(frame_block, frame_area);

                // layout
                let vert_layout = Layout::vertical([
                    Constraint::Length(12 + 1),
                    Constraint::Length(7 + 1),
                    Constraint::Fill(1),
                ])
                .split(frame_inner);
                let table_area = vert_layout[0];
                let info_area = vert_layout[1];
                let bottom = vert_layout[2];
                let hor_layout =
                    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(bottom);
                let left = hor_layout[0];
                let right = hor_layout[1];

                // table
                let table = Table::default()
                    .white()
                    .on_black()
                    .rows(self.table_rows.iter().cloned().map(Row::new))
                    .widths([
                        Constraint::Length(30),
                        Constraint::Length(6),
                        Constraint::Fill(1),
                    ])
                    .column_spacing(1)
                    .header(Row::new(["timestamp", "verb", "request uri"]).underlined())
                    .highlight_style(Style::new().black().on_gray());
                frame.render_stateful_widget(table, table_area, &mut self.table_state);

                // info
                let info_block = Block::new()
                    .title("Request Info")
                    .borders(Borders::TOP)
                    .border_type(BorderType::Rounded)
                    .padding(Padding::left(1));
                let info_inner = info_block.inner(info_area);
                frame.render_widget(info_block, info_area);
                let info_text = match event {
                    Some(event) => format!(
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
                            .map(|ob| ob.to_string())
                            .unwrap_or_else(|| "N/A".to_string()),
                        event.user.username,
                        event
                            .impersonated_user
                            .as_ref()
                            .map(|imp_user| imp_user.username.as_str())
                            .unwrap_or("N/A"),
                        event.user_agent.as_deref().unwrap_or("N/A"),
                        event
                            .source_ips
                            .as_ref()
                            .map(|ips| ips.iter().map(|ip| ip.to_string()).collect::<Vec<_>>())
                            .map(|ips| ips.join(", "))
                            .unwrap_or("N/A".to_string())
                    ),
                    None => String::new(),
                };
                frame.render_widget(Paragraph::new(info_text), info_inner);

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

                // left
                let left_text = event
                    .map(|event| {
                        event
                            .request_object
                            .as_ref()
                            .map(|req| format!("{:#}", req))
                            .unwrap_or_default()
                            .to_string()
                    })
                    .unwrap_or_default();
                frame.render_widget(
                    Paragraph::new(left_text)
                        .wrap(Wrap { trim: false })
                        .scroll((self.scroll_position, 0))
                        .white()
                        .on_black(),
                    left_inner,
                );

                // right
                let right_text = event
                    .map(|event| {
                        event
                            .response_object
                            .as_ref()
                            .map(|req| format!("{:#}", req))
                            .unwrap_or_default()
                            .to_string()
                    })
                    .unwrap_or_default();
                frame.render_widget(
                    Paragraph::new(right_text)
                        .wrap(Wrap { trim: false })
                        .scroll((self.scroll_position, 0))
                        .white()
                        .on_black(),
                    right_inner,
                );
            })
            .expect("failed to draw frame");
    }

    fn previous(&mut self) {
        let index = self.table_state.selected().map(|i| i.saturating_sub(1));
        self.table_state.select(index);
        self.scroll_position = 0;
    }

    fn next(&mut self) {
        let index = self
            .table_state
            .selected()
            .map(|i| (i + 1).min(self.table_rows.len() - 1));
        self.table_state.select(index);
        self.scroll_position = 0;
    }

    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(3);
    }

    fn scroll_down(&mut self) {
        self.scroll_position += 3;
    }
}
