//! Commands from the connection manager back to the application shell.

use std::{cell::RefCell, rc::Rc};

use wisp_core::ConnectionId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellCommand {
    Connect(ConnectionId),
    Edit(ConnectionId),
    NewConnection,
}

#[derive(Clone, Default)]
pub struct ShellCommandSender(Rc<RefCell<Vec<ShellCommand>>>);

impl ShellCommandSender {
    pub fn push(&self, command: ShellCommand) {
        self.0.borrow_mut().push(command);
    }

    pub fn drain(&self) -> Vec<ShellCommand> {
        self.0.borrow_mut().drain(..).collect()
    }
}
