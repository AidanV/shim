use portable_pty::{CommandBuilder, PtySize, PtySystem, native_pty_system};
use std::process::{Command, Output};

pub fn run(command: String) -> Option<Output> {
    let mut split_command = command.split_whitespace();
    let program = split_command.next()?;
    Command::new(program).args(split_command).output().ok()
}
