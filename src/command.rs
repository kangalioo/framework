use crate::context::Context;
use crate::utils::IdMap;
use crate::{DefaultData, DefaultError};

use serenity::futures::future::BoxFuture;
use serenity::model::channel::Message;

use std::collections::HashSet;
use std::fmt;

pub type StringHook<D = DefaultData, E = DefaultError> =
    fn(ctx: &Context<D, E>, msg: &Message) -> BoxFuture<'static, Option<String>>;
pub type StringsHook<D = DefaultData, E = DefaultError> =
    fn(ctx: &Context<D, E>, msg: &Message) -> BoxFuture<'static, Vec<String>>;

pub type CommandMap<D = DefaultData, E = DefaultError> = IdMap<String, CommandId, Command<D, E>>;

pub type CommandResult<T = (), E = DefaultError> = std::result::Result<T, E>;
pub type CommandFn<D = DefaultData, E = DefaultError> =
    fn(ctx: Context<D, E>, msg: Message) -> BoxFuture<'static, CommandResult<(), E>>;

pub type CommandConstructor<D = DefaultData, E = DefaultError> = fn() -> Command<D, E>;

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommandId(pub(crate) usize);

impl CommandId {
    pub fn into_usize(self) -> usize {
        self.0
    }
}

impl<D, E> From<CommandConstructor<D, E>> for CommandId {
    fn from(f: CommandConstructor<D, E>) -> Self {
        Self(f as usize)
    }
}

#[derive(Clone)]
pub struct Command<D = DefaultData, E = DefaultError> {
    pub id: CommandId,
    pub function: CommandFn<D, E>,
    pub names: Vec<String>,
    pub subcommands: HashSet<CommandId>,
    pub description: Option<String>,
    pub dynamic_description: Option<StringHook>,
    pub usage: Option<String>,
    pub dynamic_usage: Option<StringHook>,
    pub examples: Vec<String>,
    pub dynamic_examples: Option<StringsHook>,
    pub help_available: bool,
}

impl<D, E> Command<D, E> {
    pub fn builder<I>(name: I) -> CommandBuilder<D, E>
    where
        I: Into<String>,
    {
        CommandBuilder::new(name)
    }
}

impl<D, E> Default for Command<D, E> {
    fn default() -> Self {
        Self {
            id: CommandId::default(),
            function: |_, _| Box::pin(async { Ok(()) }),
            names: Vec::default(),
            subcommands: HashSet::default(),
            description: None,
            dynamic_description: None,
            usage: None,
            dynamic_usage: None,
            examples: Vec::default(),
            dynamic_examples: None,
            help_available: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandBuilder<D = DefaultData, E = DefaultError> {
    inner: Command<D, E>,
}

impl<D, E> Default for CommandBuilder<D, E> {
    fn default() -> Self {
        Self {
            inner: Command::default(),
        }
    }
}

impl<D, E> CommandBuilder<D, E> {
    pub fn new<I>(name: I) -> Self
    where
        I: Into<String>,
    {
        Self::default().name(name)
    }

    pub fn name<I>(mut self, name: I) -> Self
    where
        I: Into<String>,
    {
        self.inner.names.push(name.into());
        self
    }

    pub fn names<I>(mut self, names: impl IntoIterator<Item = I>) -> Self
    where
        I: Into<String>,
    {
        self.inner.names.clear();

        for name in names {
            self = self.name(name);
        }

        self
    }

    pub fn function(mut self, f: CommandFn<D, E>) -> Self {
        self.inner.function = f;
        self
    }

    pub fn subcommand(mut self, subcommand: CommandConstructor<D, E>) -> Self {
        self.inner.subcommands.insert(CommandId::from(subcommand));
        self
    }

    pub fn description<I>(mut self, description: I) -> Self
    where
        I: Into<String>,
    {
        self.inner.description = Some(description.into());
        self
    }

    pub fn dynamic_description(mut self, hook: StringHook) -> Self {
        self.inner.dynamic_description = Some(hook);
        self
    }

    pub fn usage<I>(mut self, usage: I) -> Self
    where
        I: Into<String>,
    {
        self.inner.usage = Some(usage.into());
        self
    }

    pub fn dynamic_usage(mut self, hook: StringHook) -> Self {
        self.inner.dynamic_usage = Some(hook);
        self
    }

    pub fn example<I>(mut self, example: I) -> Self
    where
        I: Into<String>,
    {
        self.inner.examples.push(example.into());
        self
    }

    pub fn examples<I>(mut self, examples: impl IntoIterator<Item = I>) -> Self
    where
        I: Into<String>,
    {
        self.inner.examples.clear();

        for example in examples {
            self = self.example(example);
        }

        self
    }

    pub fn dynamic_examples(mut self, hook: StringsHook) -> Self {
        self.inner.dynamic_examples = Some(hook);
        self
    }

    pub fn build(self) -> Command<D, E> {
        self.inner
    }
}

impl<D, E> fmt::Debug for Command<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("id", &self.id)
            .field("function", &"<fn>")
            .field("names", &self.names)
            .field("subcommands", &self.subcommands)
            .field("description", &self.description)
            .field("dynamic_description", &"<fn>")
            .field("usage", &self.usage)
            .field("dynamic_usage", &"<fn>")
            .field("examples", &self.examples)
            .field("dynamic_examples", &"<fn>")
            .field("help_available", &self.help_available)
            .finish()
    }
}
