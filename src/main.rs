mod shell;

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

#[derive(Debug, Default)]
struct Model {
    counter: i32,
    running_state: RunningState,
    outputs: Vec<Output>,
    viewing_output: usize,
    command: String,
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
    Reset,
    Enter,
    Quit,
    NewerOutput,
    OlderOutput,
    AppendCommandChar(char),
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
            "{}/{}",
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
    frame.render_widget(
        Paragraph::new(format!("❯ {}", model.command)).block(Block::bordered().title(path)),
        layout[2],
    );
    frame.render_stateful_widget(Scrollbar::default(), layout[2], scrollbar_state);
}

/// Convert Event to Message
///
/// We don't need to pass in a `model` to this function in this example
/// but you might need it as your project evolves
fn handle_event(_: &Model) -> color_eyre::Result<Option<Message>> {
    if event::poll(Duration::from_millis(250))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                return Ok(handle_key(key));
            }
        }
    }
    Ok(None)
}

fn handle_key(key: event::KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('i') => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Some(Message::NewerOutput)
            } else {
                None
            }
        }
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::OlderOutput)
        }
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Enter => Some(Message::Enter),
        KeyCode::Char(c) => Some(Message::AppendCommandChar(c)),
        _ => None,
    }
}

fn update(model: &mut Model, msg: Message) -> Option<Message> {
    match msg {
        Message::Down => {
            model.counter += 1;
            if model.counter > 50 {
                return Some(Message::Reset);
            }
        }
        Message::Up => {
            model.counter -= 1;
            if model.counter < -50 {
                return Some(Message::Reset);
            }
        }
        Message::Reset => model.counter = 0,
        Message::Enter => {
            if let Some(output) = run(model.command.clone()) {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    model.outputs.push(Output {
                        text: "blah".into(),
                        program: s,
                        scrollbar_state: ScrollbarState::default(),
                    });
                    model.viewing_output = model.outputs.len() - 1;
                }
            }
        }
        Message::Quit => {
            // You can handle cleanup and exit here
            model.running_state = RunningState::Done;
        }
        Message::NewerOutput => {
            if model.outputs.is_empty() {
                model.viewing_output = 0;
            } else {
                model.viewing_output = model.viewing_output.saturating_sub(1);
            }
        }
        Message::OlderOutput => {
            if model.outputs.is_empty() {
                model.viewing_output = 0;
            } else {
                model.viewing_output = min(
                    model.outputs.len() - 1,
                    model.viewing_output.saturating_add(1),
                );
            }
        }
        Message::AppendCommandChar(c) => model.command.push(c),
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
