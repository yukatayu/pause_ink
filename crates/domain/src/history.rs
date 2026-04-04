use thiserror::Error;

pub const DEFAULT_HISTORY_DEPTH: usize = 256;

pub trait Command<S> {
    fn apply(&self, state: &mut S) -> Result<(), CommandError>;
    fn undo(&self, state: &mut S) -> Result<(), CommandError>;
}

pub struct CommandBatch<S> {
    commands: Vec<Box<dyn Command<S>>>,
}

impl<S> CommandBatch<S> {
    pub fn new(commands: Vec<Box<dyn Command<S>>>) -> Self {
        Self { commands }
    }
}

impl<S> Command<S> for CommandBatch<S> {
    fn apply(&self, state: &mut S) -> Result<(), CommandError> {
        let mut applied = 0usize;

        for command in &self.commands {
            if let Err(error) = command.apply(state) {
                for undo_command in self.commands[..applied].iter().rev() {
                    undo_command.undo(state)?;
                }
                return Err(error);
            }
            applied += 1;
        }

        Ok(())
    }

    fn undo(&self, state: &mut S) -> Result<(), CommandError> {
        for command in self.commands.iter().rev() {
            command.undo(state)?;
        }
        Ok(())
    }
}

pub struct CommandHistory<S> {
    limit: usize,
    undo_stack: Vec<Box<dyn Command<S>>>,
    redo_stack: Vec<Box<dyn Command<S>>>,
}

impl<S> CommandHistory<S> {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn apply(
        &mut self,
        state: &mut S,
        command: Box<dyn Command<S>>,
    ) -> Result<(), CommandError> {
        command.apply(state)?;
        self.redo_stack.clear();

        if self.limit > 0 {
            self.undo_stack.push(command);
            self.enforce_limit();
        }

        Ok(())
    }

    pub fn undo(&mut self, state: &mut S) -> Result<bool, CommandError> {
        let Some(command) = self.undo_stack.pop() else {
            return Ok(false);
        };

        command.undo(state)?;
        if self.limit > 0 {
            self.redo_stack.push(command);
        }
        Ok(true)
    }

    pub fn redo(&mut self, state: &mut S) -> Result<bool, CommandError> {
        let Some(command) = self.redo_stack.pop() else {
            return Ok(false);
        };

        command.apply(state)?;
        if self.limit > 0 {
            self.undo_stack.push(command);
            self.enforce_limit();
        }
        Ok(true)
    }

    fn enforce_limit(&mut self) {
        if self.undo_stack.len() > self.limit {
            self.undo_stack.remove(0);
        }
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct CommandError {
    message: String,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
