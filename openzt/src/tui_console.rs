//! Terminal User Interface (TUI) console for OpenZT
//!
//! This module provides a unified TUI that combines logging output and Lua command input.
//! It uses ratatui for rendering and crossterm for terminal handling.

use crate::logging::LogLevel;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use tracing::Level;

// Maximum number of log entries to keep in memory
const MAX_LOG_ENTRIES: usize = 1000;

// Maximum number of command history entries
const MAX_COMMAND_HISTORY: usize = 100;

/// TUI configuration section
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TuiConfig {
    /// Enable TUI (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Show logs in TUI (default: true)
    #[serde(default = "default_true")]
    pub show_logs: bool,

    /// Minimum log level for TUI display (default: Info)
    #[serde(default)]
    pub log_level: TuiLogLevel,

    /// Number of lines scrolled per mouse wheel tick (default: 1)
    #[serde(default = "default_mouse_scroll_lines")]
    pub mouse_scroll_lines: usize,
}

/// Log level setting for TUI
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TuiLogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl TuiLogLevel {
    /// Convert to tracing's Level
    pub fn to_level(self) -> Level {
        match self {
            TuiLogLevel::Trace => Level::TRACE,
            TuiLogLevel::Debug => Level::DEBUG,
            TuiLogLevel::Info => Level::INFO,
            TuiLogLevel::Warn => Level::WARN,
            TuiLogLevel::Error => Level::ERROR,
        }
    }

    /// Convert from LoggingConfig's LogLevel
    pub fn from_log_level(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => TuiLogLevel::Trace,
            LogLevel::Debug => TuiLogLevel::Debug,
            LogLevel::Info => TuiLogLevel::Info,
            LogLevel::Warn => TuiLogLevel::Warn,
            LogLevel::Error => TuiLogLevel::Error,
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        TuiConfig {
            enabled: true,
            show_logs: true,
            log_level: TuiLogLevel::Info,
            mouse_scroll_lines: default_mouse_scroll_lines(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_mouse_scroll_lines() -> usize {
    1
}

/// A single log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: Level,
    pub message: String,
}

/// Shared TUI state
#[derive(Debug)]
pub struct TuiState {
    /// Log buffer
    pub log_buffer: VecDeque<LogEntry>,
    /// Command output buffer
    pub command_output: VecDeque<String>,
    /// Current input line
    pub input: String,
    /// Command history
    pub command_history: VecDeque<String>,
    /// History navigation index (None = current input)
    pub history_index: Option<usize>,
    /// Log scroll offset
    pub log_scroll: usize,
    /// Command output scroll offset
    pub output_scroll: usize,
    /// Whether TUI is running
    pub running: bool,
    /// Minimum log level to display
    pub min_log_level: Level,
    /// Whether to show logs
    pub show_logs: bool,
    /// Lines scrolled per mouse wheel tick
    pub mouse_scroll_lines: usize,
}

impl Default for TuiState {
    fn default() -> Self {
        TuiState {
            log_buffer: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            command_output: VecDeque::new(),
            input: String::new(),
            command_history: VecDeque::with_capacity(MAX_COMMAND_HISTORY),
            history_index: None,
            log_scroll: 0,
            output_scroll: 0,
            running: false,
            min_log_level: Level::INFO,
            show_logs: true,
            mouse_scroll_lines: default_mouse_scroll_lines(),
        }
    }
}

/// Global TUI state
static GLOBAL_TUI_STATE: LazyLock<Arc<Mutex<TuiState>>> = LazyLock::new(|| {
    Arc::new(Mutex::new(TuiState::default()))
});

/// Custom writer that captures log output for the TUI
pub struct TuiWriter;

impl std::io::Write for TuiWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        add_log_entry(&s);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Add a log entry to the TUI buffer
fn add_log_entry(message: &str) {
    let mut state = GLOBAL_TUI_STATE.lock().unwrap();

    if !state.show_logs {
        return;
    }

    // Parse the message to extract level and actual message
    // tracing format: "2024-01-01T12:00:00.000Z LEVEL module: message"
    // or simplified format like "LEVEL message"
    let (level, actual_message) = if let Some(rest) = message.strip_prefix("TRACE ") {
        (Level::TRACE, rest)
    } else if let Some(rest) = message.strip_prefix("DEBUG ") {
        (Level::DEBUG, rest)
    } else if let Some(rest) = message.strip_prefix("INFO ") {
        (Level::INFO, rest)
    } else if let Some(rest) = message.strip_prefix("WARN ") {
        (Level::WARN, rest)
    } else if let Some(rest) = message.strip_prefix("ERROR ") {
        (Level::ERROR, rest)
    } else {
        // Default to INFO if we can't parse
        (Level::INFO, message)
    };

    if level > state.min_log_level {
        return;
    }

    // Extract timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let timestamp = format!("{:02}:{:02}:{:02}",
        (secs % 86400) / 3600,
        (secs % 3600) / 60,
        secs % 60
    );

    state.log_buffer.push_back(LogEntry {
        timestamp,
        level,
        message: actual_message.trim().to_string(),
    });

    // Trim buffer if needed
    while state.log_buffer.len() > MAX_LOG_ENTRIES {
        state.log_buffer.pop_front();
    }
}

/// Get the TUI writer for use with tracing
pub fn get_tui_writer() -> TuiWriter {
    TuiWriter
}

/// Initialize the TUI with the given configuration
pub fn init(config: &TuiConfig) -> anyhow::Result<()> {
    if !config.enabled {
        return Ok(());
    }

    // Update the state with config values
    {
        let mut state = GLOBAL_TUI_STATE.lock().unwrap();
        state.min_log_level = config.log_level.to_level();
        state.show_logs = config.show_logs;
        state.mouse_scroll_lines = config.mouse_scroll_lines;
        state.running = true;
    }

    // Spawn the TUI thread
    std::thread::spawn(|| {
        if let Err(e) = run_tui() {
            eprintln!("TUI error: {}", e);
        }
    });

    Ok(())
}

/// Run the TUI event loop
fn run_tui() -> anyhow::Result<()> {
    #[cfg(not(feature = "tui"))]
    compile_error!("tui feature must be enabled to use TUI");

    // Check if TUI is enabled
    {
        let state = GLOBAL_TUI_STATE.lock().unwrap();
        if !state.running {
            return Ok(());
        }
    }

    // Initialize terminal
    #[cfg(feature = "tui")]
    {
        use ratatui::{
            backend::CrosstermBackend,
            crossterm::{
                event::{DisableMouseCapture, EnableMouseCapture},
                execute,
                terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
            },
            Terminal,
        };

        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = run_tui_inner(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    #[cfg(not(feature = "tui"))]
    Ok(())
}

/// Inner TUI loop
#[cfg(feature = "tui")]
fn run_tui_inner(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
) -> anyhow::Result<()> {
    use ratatui::{
        crossterm::event::{self, Event},
        layout::{Alignment, Constraint, Direction, Layout, Rect},
        style::{Color, Style},
        text::{Line, Span, Text},
        widgets::{Block, Borders, Paragraph, Wrap},
    };
    use std::time::Duration;

    let mut last_tick = std::time::Instant::now();
    let tick_rate = Duration::from_millis(100);

    // Updated at the top of each tick before event handling; tracks which pane the mouse is over
    let mut last_pane_rects: [Rect; 2];

    loop {
        // Check if still running
        {
            let state = GLOBAL_TUI_STATE.lock().unwrap();
            if !state.running {
                return Ok(());
            }
        }

        // Refresh pane rects for mouse hit-testing
        {
            let term_size = terminal.size()?;
            let term_rect = Rect::new(0, 0, term_size.width, term_size.height);
            let computed = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(term_rect);
            last_pane_rects = [computed[0], computed[1]];
        }

        // Draw UI
        terminal.draw(|f| {
            let size = f.area();

            // Split screen: 3-way split for logs, command output, and input
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([
                    Constraint::Percentage(40),  // Logs
                    Constraint::Min(10),         // Command output (at least 10 lines)
                    Constraint::Length(3),       // Input
                ])
                .split(size);

            // Get log entries for the logs section — newest at bottom, like a terminal.
            // 1. Reverse so we start from the most recent entry.
            // 2. Skip entries the user has scrolled past.
            // 3. Take only as many lines as the pane can display.
            // 4. Reverse again so the slice is in chronological order (oldest → newest).
            let (log_entries, min_level) = {
                let state = GLOBAL_TUI_STATE.lock().unwrap();
                let visible = chunks[0].height.saturating_sub(2) as usize;
                let mut logs: Vec<LogEntry> = state.log_buffer.iter()
                    .filter(|e| e.level <= state.min_log_level)
                    .rev()
                    .skip(state.log_scroll)
                    .take(visible)
                    .cloned()
                    .collect();
                logs.reverse();
                let min_lvl = state.min_log_level;
                (logs, min_lvl)
            };

            // Render logs section
            let log_lines: Vec<Line> = log_entries.iter()
                .filter(|e| e.level <= min_level)
                .map(|e| {
                    let level_color = match e.level {
                        Level::ERROR => Color::Red,
                        Level::WARN => Color::Yellow,
                        Level::INFO => Color::Cyan,
                        Level::DEBUG => Color::Gray,
                        Level::TRACE => Color::DarkGray,
                    };
                    Line::from(vec![
                        Span::styled(format!("[{}] ", e.timestamp), Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{:<5} ", e.level), Style::default().fg(level_color)),
                        Span::raw(e.message.clone()),
                    ])
                })
                .collect();

            let log_text = Text::from(log_lines);
            let log_paragraph = Paragraph::new(log_text)
                .block(Block::default().borders(Borders::ALL).title("Logs"))
                .wrap(Wrap { trim: true });
            f.render_widget(log_paragraph, chunks[0]);

            // Get command output entries — newest at bottom, like a terminal.
            // 1. Reverse the deque so we start from the most recent entry.
            // 2. Skip any entries the user has scrolled past.
            // 3. Take only as many lines as the pane can display.
            // 4. Reverse again so the slice is in chronological order (oldest → newest).
            let output_entries = {
                let state = GLOBAL_TUI_STATE.lock().unwrap();
                let visible = chunks[1].height.saturating_sub(2) as usize;
                let mut entries: Vec<String> = state.command_output.iter()
                    .rev()
                    .skip(state.output_scroll)
                    .take(visible)
                    .cloned()
                    .collect();
                entries.reverse();
                entries
            };

            // Render command output section
            let output_lines: Vec<Line> = output_entries.iter()
                .map(|s| Line::from(s.as_str()))
                .collect();

            let output_text = Text::from(output_lines);
            let output_paragraph = Paragraph::new(output_text)
                .block(Block::default().borders(Borders::ALL).title("Command Output"))
                .wrap(Wrap { trim: true });
            f.render_widget(output_paragraph, chunks[1]);

            // Render input line
            let (input_text, cursor_pos) = {
                let state = GLOBAL_TUI_STATE.lock().unwrap();
                (state.input.clone(), state.input.len())
            };

            let input_paragraph = Paragraph::new(Line::from(vec![
                Span::styled("Lua> ", Style::default().fg(Color::Green)),
                Span::raw(input_text.clone()),
            ]))
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Left);

            f.render_widget(input_paragraph, chunks[2]);
            f.set_cursor_position(ratatui::layout::Position::new(
                chunks[2].x + 5 + cursor_pos as u16,
                chunks[2].y + 1,
            ));
        })?;

        // Clamp scroll offsets so neither pane can scroll past its content
        {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();

            let log_visible = last_pane_rects[0].height.saturating_sub(2) as usize;
            let log_total: usize = state.log_buffer
                .iter()
                .filter(|e| e.level <= state.min_log_level)
                .count();
            state.log_scroll = state.log_scroll.min(log_total.saturating_sub(log_visible));

            let out_visible = last_pane_rects[1].height.saturating_sub(2) as usize;
            let out_total = state.command_output.len();
            state.output_scroll = state.output_scroll.min(out_total.saturating_sub(out_visible));
        }

        // Handle input with timeout
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => handle_key_event(key),
                Event::Mouse(mouse) => handle_mouse_event(mouse, last_pane_rects),
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
    }
}

/// Handle keyboard input
#[cfg(feature = "tui")]
fn handle_key_event(key: ratatui::crossterm::event::KeyEvent) {
    use ratatui::crossterm::event::{KeyCode, KeyEventKind};

    if key.kind != KeyEventKind::Press {
        return;
    }

    match key.code {
        KeyCode::Char(c) => {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();
            state.input.push(c);
        }
        KeyCode::Backspace => {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();
            state.input.pop();
        }
        KeyCode::Enter => {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();
            if !state.input.is_empty() {
                let input = state.input.clone();
                state.input.clear();
                state.history_index = None;

                // Add to history
                state.command_history.push_back(input.clone());
                while state.command_history.len() > MAX_COMMAND_HISTORY {
                    state.command_history.pop_front();
                }

                // Echo the command to the output pane and jump to the bottom
                state.command_output.push_back(format!("Lua> {}", input));
                while state.command_output.len() > MAX_LOG_ENTRIES {
                    state.command_output.pop_front();
                }
                state.output_scroll = 0;

                // Add to command queue for execution
                drop(state);
                add_command_to_queue(input);
            }
        }
        KeyCode::Up => {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();
            if state.command_history.is_empty() {
                return;
            }

            let new_index = match state.history_index {
                None => Some(state.command_history.len() - 1),
                Some(i) if i > 0 => Some(i - 1),
                _ => return,
            };

            state.history_index = new_index;
            if let Some(idx) = new_index {
                if let Some(cmd) = state.command_history.get(idx) {
                    state.input = cmd.clone();
                }
            }
        }
        KeyCode::Down => {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();
            match state.history_index {
                None => {}
                Some(i) => {
                    if i + 1 < state.command_history.len() {
                        state.history_index = Some(i + 1);
                        if let Some(cmd) = state.command_history.get(i + 1) {
                            state.input = cmd.clone();
                        }
                    } else {
                        state.history_index = None;
                        state.input.clear();
                    }
                }
            }
        }
        KeyCode::PageUp => {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();
            state.log_scroll = state.log_scroll.saturating_add(10);
        }
        KeyCode::PageDown => {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();
            state.log_scroll = state.log_scroll.saturating_sub(10);
        }
        KeyCode::Esc => {
            let mut state = GLOBAL_TUI_STATE.lock().unwrap();
            state.running = false;
        }
        _ => {}
    }
}

/// Handle mouse events (scroll wheel in either pane)
#[cfg(feature = "tui")]
fn handle_mouse_event(
    mouse: ratatui::crossterm::event::MouseEvent,
    pane_rects: [ratatui::layout::Rect; 2],
) {
    use ratatui::crossterm::event::MouseEventKind;

    let in_logs = rect_contains(pane_rects[0], mouse.column, mouse.row);
    let in_output = rect_contains(pane_rects[1], mouse.column, mouse.row);
    if !in_logs && !in_output {
        return;
    }

    let mut state = GLOBAL_TUI_STATE.lock().unwrap();
    let scroll_amount = state.mouse_scroll_lines;
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if in_logs {
                state.log_scroll = state.log_scroll.saturating_add(scroll_amount);
            }
            if in_output {
                state.output_scroll = state.output_scroll.saturating_add(scroll_amount);
            }
        }
        MouseEventKind::ScrollDown => {
            if in_logs {
                state.log_scroll = state.log_scroll.saturating_sub(scroll_amount);
            }
            if in_output {
                state.output_scroll = state.output_scroll.saturating_sub(scroll_amount);
            }
        }
        _ => {}
    }
}

fn rect_contains(rect: ratatui::layout::Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

/// Add a command to the queue for execution on the game thread
fn add_command_to_queue(command: String) {
    // Reuse the existing command console queue
    crate::command_console::add_to_command_queue(command);
}

/// Add command output to the TUI display
pub fn add_command_output(output: String) {
    let mut state = GLOBAL_TUI_STATE.lock().unwrap();
    for line in output.lines() {
        state.command_output.push_back(line.to_string());
    }
    // Trim if needed
    while state.command_output.len() > MAX_LOG_ENTRIES {
        state.command_output.pop_front();
    }
    // Jump to bottom so results are always visible
    state.output_scroll = 0;
}

/// Shut down the TUI
pub fn shutdown() {
    let mut state = GLOBAL_TUI_STATE.lock().unwrap();
    state.running = false;
}
