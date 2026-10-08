//! Commands from the connection manager back to the application shell.

use std::{cell::RefCell, rc::Rc};

use wisp_core::{ConnectionFormDraft, ConnectionId};

#[derive(Debug, Clone)]
pub enum ShellCommand {
    Connect(ConnectionId),
    ConnectNewWindow(ConnectionId),
    Edit(ConnectionId),
    NewConnection,
    NewConnectionFromDraft(Box<ConnectionFormDraft>),
    FormSaved {
        id: ConnectionId,
        connect: bool,
    },
    FormCancelled,
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
