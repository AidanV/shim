mod shell;

use std::{cmp::min, env, time::Duration};

use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::Position;
use ratatui::{
    Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Paragraph},
};
use shell::run;

#[derive(Debug, PartialEq)]
enum Cursor {
    CommandLine(u16),
    OutputBuffer(u16, u16),
}

impl Cursor {
    fn left(&mut self) {
        match self {
            Cursor::CommandLine(x) => *x = x.saturating_sub(1),
            Cursor::OutputBuffer(x, _) => *x = x.saturating_sub(1),
        }
    }
    fn right(&mut self) {
        match self {
            Cursor::CommandLine(x) => *x = x.saturating_add(1),
            Cursor::OutputBuffer(x, _) => *x = x.saturating_add(1),
        }
    }

    fn right_capped(&mut self, max: u16) {
        match self {
            Cursor::CommandLine(x) => *x = min(x.saturating_add(1), max),
            Cursor::OutputBuffer(x, _) => *x = x.saturating_add(1),
        }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Cursor::CommandLine(0)
    }
}

#[derive(Debug, Default, PartialEq)]
enum Mode {
    #[default]
    Insert,
    Normal,
}

#[derive(Debug, Default)]
struct Model {
    cursor: Cursor,
    mode: Mode,
    running_state: RunningState,
    outputs: Vec<Output>,
    previous_commands: Vec<String>,
    viewing_output: usize,
    current_command: String,
    viewing_command: Option<usize>,
    height: u16,
}

impl Model {
    fn get_command_len(&self) -> u16 {
        match self
            .viewing_command
            .and_then(|i| self.previous_commands.get(i))
        {
            Some(s) => s.len() as u16,
            None => self.current_command.len() as u16,
        }
    }
}

#[derive(Debug, Default)]
struct Output {
    command: String,
    stdout: String,
    scroll: (u16, u16),
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
    WriteCommandChar(char),
    Normal,
    InsertBefore,
    InsertAfter,
    Backspace,
    OutCommand,
    InCommand,
    ScrollDown,
    ScrollUp,
    Left,
    Right,
    InsertBeforeLine,
    InsertAfterLine,
}

impl Message {
    fn is_editing_command(&self) -> bool {
        matches!(
            self,
            Self::Submit | Self::WriteCommandChar(_) | Self::Backspace
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

    model.height = layout[1].height.saturating_sub(2); // for the borders

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

    let (program, text, scroll) = model
        .outputs
        .get_mut(model.viewing_output)
        .map(|o| (&o.stdout[..], &o.command[..], o.scroll))
        .unwrap_or(("", "", (0, 0)));
    frame.render_widget(
        Paragraph::new(program)
            .scroll(scroll)
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

    match model.cursor {
        Cursor::CommandLine(x) => {
            frame.set_cursor_position(Position::new(layout[2].x + 3 + x, layout[2].y + 1))
        }
        Cursor::OutputBuffer(x, y) => {
            frame.set_cursor_position(Position::new(layout[1].x + 1 + x, layout[1].y + 1 + y))
        }
    }
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
            KeyCode::Char(c) => Some(Message::WriteCommandChar(c)),
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
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::ScrollUp)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::ScrollDown)
            }
            KeyCode::Char('i') => Some(Message::InsertBefore),
            KeyCode::Char('a') => Some(Message::InsertAfter),
            KeyCode::Char('I') => Some(Message::InsertBeforeLine),
            KeyCode::Char('A') => Some(Message::InsertAfterLine),
            KeyCode::Char('h') => Some(Message::Left),
            KeyCode::Char('j') => Some(Message::Down),
            KeyCode::Char('k') => Some(Message::Up),
            KeyCode::Char('l') => Some(Message::Right),
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
        Message::Down => match model.cursor {
            Cursor::CommandLine(_) => {}
            Cursor::OutputBuffer(x, y) => {
                if y + 1 >= model.height {
                    model.cursor = Cursor::CommandLine(x);
                } else {
                    model.cursor = Cursor::OutputBuffer(x, y + 1)
                }
            }
        },
        Message::Up => match model.cursor {
            Cursor::CommandLine(x) => {
                model.cursor = Cursor::OutputBuffer(x, model.height.saturating_sub(1))
            }
            Cursor::OutputBuffer(x, y) => {
                model.cursor = Cursor::OutputBuffer(x, y.saturating_sub(1))
            }
        },
        Message::Left => {
            model.cursor.left();
        }
        Message::Right => {
            let max = match model.cursor {
                Cursor::CommandLine(_) => match model.viewing_command {
                    Some(i) => model.previous_commands.get(i).map(|s| s.len()).unwrap_or(0),
                    None => model.current_command.len(),
                },
                Cursor::OutputBuffer(_, y) => model
                    .outputs
                    .get(model.viewing_output)
                    .map(|o| {
                        let s = o.stdout.lines().nth((y + o.scroll.0) as usize).unwrap();
                        s.len()
                    })
                    .unwrap_or(0),
            };
            model.cursor.right_capped(max as u16);
        }
        Message::Submit => {
            if let Some(output) = run(model.current_command.clone()) {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    model.outputs.push(Output {
                        command: model.current_command.clone(),
                        stdout: s.clone(),
                        scroll: ((s.lines().count() as u16).saturating_sub(model.height), 0),
                    });
                    model.viewing_output = model.outputs.len() - 1;
                }
            }
            model.previous_commands.push(model.current_command.clone());
            model.viewing_command = None;
            model.current_command.clear();
            model.cursor = Cursor::CommandLine(0);
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
        Message::WriteCommandChar(c) => {
            match model.cursor {
                Cursor::CommandLine(x) => model.current_command.insert(x as usize, c),
                Cursor::OutputBuffer(_, _) => panic!(
                    "not supposed to write character to command when cursor is in output buffer"
                ),
            }
            model.cursor.right();
        }
        Message::Normal => model.mode = Mode::Normal,
        Message::InsertBefore => {
            model.mode = Mode::Insert;
            let x = match model.cursor {
                Cursor::CommandLine(x) => x,
                Cursor::OutputBuffer(x, _) => x,
            };
            model.cursor = Cursor::CommandLine(min(model.get_command_len(), x))
        }
        Message::InsertAfter => {
            model.mode = Mode::Insert;
            let x = match model.cursor {
                Cursor::CommandLine(x) => x,
                Cursor::OutputBuffer(x, _) => x,
            };
            model.cursor = Cursor::CommandLine(min(model.get_command_len(), x + 1))
        }
        Message::Backspace => {
            model.cursor.left();
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
        Message::ScrollDown => {
            if let Some(output) = model.outputs.get_mut(model.viewing_output) {
                let (vert, horiz) = output.scroll;
                output.scroll = (vert.saturating_add(10), horiz);
            }
        }
        Message::ScrollUp => {
            if let Some(output) = model.outputs.get_mut(model.viewing_output) {
                let (vert, horiz) = output.scroll;
                output.scroll = (vert.saturating_sub(10), horiz);
            }
        }
        Message::InsertBeforeLine => {
            model.mode = Mode::Insert;
            model.cursor = Cursor::CommandLine(0);
        }
        Message::InsertAfterLine => {
            model.mode = Mode::Insert;
            model.cursor = Cursor::CommandLine(model.get_command_len())
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
