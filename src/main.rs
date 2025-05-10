mod shell;

use std::default;
use std::{cmp::min, env, time::Duration};

use ratatui::crossterm::event::KeyModifiers;
use ratatui::widgets::{Scrollbar, ScrollbarState};
use ratatui::{
    Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Paragraph},
};
use shell::run;

#[derive(Debug, Default, PartialEq)]
enum Mode {
    #[default]
    Insert,
    Normal,
}

#[derive(Debug, Default)]
struct Model {
    mode: Mode,
    running_state: RunningState,
    outputs: Vec<Output>,
    previous_commands: Vec<String>,
    viewing_output: usize,
    current_command: String,
    viewing_command: Option<usize>,
}

#[derive(Debug, Default)]
struct Output {
    text: String,
    program: String,
    scrollbar_state: ScrollbarState,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum RunningState {
    #[default]
    Running,
    Done,
}

#[derive(PartialEq)]
enum Message {
    Down,
    Up,
    Submit,
    Quit,
    NextOutput,
    PreviousOutput,
    AppendCommandChar(char),
    Normal,
    InsertBefore,
    InsertAfter,
    Backspace,
    OutCommand,
    InCommand,
}

impl Message {
    fn is_editing_command(&self) -> bool {
        matches!(
            self,
            Self::Submit | Self::AppendCommandChar(_) | Self::Backspace
        )
    }
}

fn main() -> color_eyre::Result<()> {
    tui::install_panic_hook();
    let mut terminal = tui::init_terminal()?;
    let mut model = Model::default();

    while model.running_state != RunningState::Done {
        // Render the current view
        terminal.draw(|f| view(&mut model, f))?;

        // Handle events and map to a Message
        let mut current_msg = handle_event(&model)?;

        // Process updates as long as they return a non-None message
        while current_msg.is_some() {
            current_msg = update(&mut model, current_msg.unwrap());
        }
    }

    tui::restore_terminal()?;
    Ok(())
}

fn view(model: &mut Model, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(frame.area());

    let path = env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(|p| p.to_string()))
        .unwrap_or("~".into());

    frame.render_widget(
        Paragraph::new(format!(
            "{:?}  {}/{}",
            model.mode,
            model.viewing_output + 1,
            model.outputs.len()
        ))
        .right_aligned(),
        layout[0],
    );

    let scrollbar_state_default = &mut ScrollbarState::default();
    let (program, text, scrollbar_state) = model
        .outputs
        .get_mut(model.viewing_output)
        .map(|o| (&o.program[..], &o.text[..], &mut o.scrollbar_state))
        .unwrap_or(("", "", scrollbar_state_default));
    frame.render_widget(
        Paragraph::new(program)
            // .scroll((28, 0))
            .block(Block::bordered().title(text)),
        layout[1],
    );

    if let Some(curr) = model.viewing_command {
        let show = model
            .previous_commands
            .get(curr)
            .cloned()
            .unwrap_or("".into());
        frame.render_widget(
            Paragraph::new(format!("❯ {}", show)).block(Block::bordered().title(path)),
            layout[2],
        );
    } else {
        frame.render_widget(
            Paragraph::new(format!("❯ {}", model.current_command))
                .block(Block::bordered().title(path)),
            layout[2],
        );
    }
    frame.render_stateful_widget(Scrollbar::default(), layout[2], scrollbar_state);
}

/// Convert Event to Message
///
/// We don't need to pass in a `model` to this function in this example
/// but you might need it as your project evolves
fn handle_event(model: &Model) -> color_eyre::Result<Option<Message>> {
    if event::poll(Duration::from_millis(250))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                return Ok(handle_key(model, key));
            }
        }
    }
    Ok(None)
}

fn handle_key(model: &Model, key: event::KeyEvent) -> Option<Message> {
    match model.mode {
        Mode::Insert => match key.code {
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::Quit)
            }
            KeyCode::Char(c) => Some(Message::AppendCommandChar(c)),
            KeyCode::Esc => Some(Message::Normal),
            KeyCode::Backspace => Some(Message::Backspace),
            KeyCode::Enter => Some(Message::Submit),
            _ => None,
        },
        Mode::Normal => match key.code {
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::NextOutput)
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::PreviousOutput)
            }
            KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::InCommand)
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::OutCommand)
            }
            KeyCode::Char('i') => Some(Message::InsertBefore),
            KeyCode::Char('a') => Some(Message::InsertAfter),
            _ => None,
        },
    }
}

fn update(model: &mut Model, msg: Message) -> Option<Message> {
    if msg.is_editing_command() {
        if let Some(curr) = model.viewing_command {
            model.current_command = model
                .previous_commands
                .get(curr)
                .cloned()
                .unwrap_or("".into());
        }
        model.viewing_command = None;
    }
    match msg {
        Message::Down => {}
        Message::Up => {}
        Message::Submit => {
            if let Some(output) = run(model.current_command.clone()) {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    model.outputs.push(Output {
                        text: model.current_command.clone(),
                        program: s,
                        scrollbar_state: ScrollbarState::default(),
                    });
                    model.viewing_output = model.outputs.len() - 1;
                }
            }
            model.previous_commands.push(model.current_command.clone());
            model.viewing_command = None;
            model.current_command.clear();
        }
        Message::Quit => {
            // You can handle cleanup and exit here
            model.running_state = RunningState::Done;
        }
        Message::NextOutput => {
            if model.outputs.is_empty() {
                model.viewing_output = 0;
            } else {
                model.viewing_output = min(
                    model.outputs.len() - 1,
                    model.viewing_output.saturating_add(1),
                );
            }
        }
        Message::PreviousOutput => {
            if model.outputs.is_empty() {
                model.viewing_output = 0;
            } else {
                model.viewing_output = model.viewing_output.saturating_sub(1);
            }
        }
        Message::AppendCommandChar(c) => model.current_command.push(c),
        Message::Normal => model.mode = Mode::Normal,
        Message::InsertBefore => model.mode = Mode::Insert,
        Message::InsertAfter => model.mode = Mode::Insert,
        Message::Backspace => {
            let _ = model.current_command.pop();
        }
        Message::OutCommand => {
            if let Some(curr) = model.viewing_command {
                model.viewing_command = Some(curr.saturating_sub(1));
            } else if !model.previous_commands.is_empty() {
                model.viewing_command = Some(model.previous_commands.len() - 1);
            }
        }
        Message::InCommand => {
            if let Some(curr) = model.viewing_command {
                if curr >= model.previous_commands.len() - 1 {
                    model.viewing_command = None;
                } else {
                    model.viewing_command = Some(curr + 1);
                }
            }
        }
    };
    None
}

mod tui {
    use ratatui::{
        Terminal,
        backend::{Backend, CrosstermBackend},
        crossterm::{
            ExecutableCommand,
            terminal::{
                EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
            },
        },
    };
    use std::{io::stdout, panic};

    pub fn init_terminal() -> color_eyre::Result<Terminal<impl Backend>> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        Ok(terminal)
    }

    pub fn restore_terminal() -> color_eyre::Result<()> {
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    pub fn install_panic_hook() {
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            stdout().execute(LeaveAlternateScreen).unwrap();
            disable_raw_mode().unwrap();
            original_hook(panic_info);
        }));
    }
}
