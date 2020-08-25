use serenity::model::channel::Message;
use serenity::prelude::{Context as SerenityContext, Mutex, RwLock};

use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;

pub mod command;
pub mod configuration;
pub mod context;
pub mod error;
pub mod group;
pub mod parse;
pub mod prelude;
pub mod utils;

use command::{CommandFn, CommandResult};
use configuration::Configuration;
use context::{Context, PrefixContext};
use error::{DispatchError, Error};

pub type DefaultData = ();
pub type DefaultError = Box<dyn StdError + Send + Sync>;

#[derive(Clone)]
pub struct Framework<D = DefaultData, E = DefaultError> {
    pub conf: Arc<Mutex<Configuration<D, E>>>,
    pub data: Arc<RwLock<D>>,
}

impl<D, E> Framework<D, E>
where
    D: Default,
{
    #[inline]
    pub fn new(conf: Configuration<D, E>) -> Self {
        Self::with_data(conf, D::default())
    }
}

impl<D, E> Framework<D, E> {
    #[inline]
    pub fn with_arc_data(conf: Configuration<D, E>, data: Arc<RwLock<D>>) -> Self {
        Self {
            conf: Arc::new(Mutex::new(conf)),
            data,
        }
    }

    #[inline]
    pub fn with_data(conf: Configuration<D, E>, data: D) -> Self {
        Self::with_arc_data(conf, Arc::new(RwLock::new(data)))
    }

    #[inline]
    pub async fn dispatch(&self, ctx: SerenityContext, msg: Message) -> Result<(), Error<E>> {
        self.dispatch_with_hook(ctx, msg, |ctx, msg, f| async move { f(ctx, msg).await })
            .await
    }

    pub async fn dispatch_with_hook<F, Fut>(
        &self,
        ctx: SerenityContext,
        msg: Message,
        hook: F,
    ) -> Result<(), Error<E>>
    where
        F: FnOnce(Context<D, E>, Message, CommandFn<D, E>) -> Fut,
        Fut: Future<Output = CommandResult<(), E>>,
    {
        let (func, group_id, command_id, command_name, prefix, args) = 'block: loop {
            let conf = self.conf.lock().await;

            if conf.blocked_entities.users.contains(&msg.author.id) {
                return Err(Error::Dispatch(DispatchError::BlockedUser(msg.author.id)));
            }

            if conf.blocked_entities.channels.contains(&msg.channel_id) {
                return Err(Error::Dispatch(DispatchError::BlockedChannel(
                    msg.channel_id,
                )));
            }

            if let Some(guild_id) = msg.guild_id {
                if conf.blocked_entities.guilds.contains(&guild_id) {
                    return Err(Error::Dispatch(DispatchError::BlockedGuild(guild_id)));
                }
            }

            let prefix_ctx = PrefixContext {
                data: self.data.clone(),
                conf: &conf,
                serenity_ctx: &ctx,
            };

            let (prefix, content) = parse::content(prefix_ctx, &msg)
                .await
                .ok_or(Error::Dispatch(DispatchError::NormalMessage))?;

            let mut segments = parse::Segments::new(&content, ' ', conf.case_insensitive);

            let mut name = segments.next().ok_or(DispatchError::PrefixOnly)?;
            let mut group = conf.groups.get_by_name(&*name);

            while let Some(g) = group {
                name = match segments.next() {
                    Some(name) => name,
                    None => {
                        if let Some(id) = g.default_command {
                            let command = &conf.commands[id];

                            break 'block (
                                command.function,
                                g.id,
                                command.id,
                                command.names[0].clone(),
                                prefix.to_string(),
                                "".to_string(),
                            );
                        }

                        return Err(Error::Dispatch(DispatchError::MissingContent));
                    }
                };

                // Check whether there's a subgroup.
                // Only assign it to `group` if it's a part of `group`'s subgroups.
                if let Some((id, aggr)) = conf.groups.get_pair(&*name) {
                    if conf.blocked_entities.groups.contains(&id) {
                        return Err(Error::Dispatch(DispatchError::BlockedGroup(id)));
                    }

                    if g.subgroups.contains(&id) {
                        group = Some(aggr);
                        continue;
                    }
                }

                // No more subgroups to be found.
                break;
            }

            // If we could not find more subgroups, `name` will be the segment
            // after the `group`. If we could not find a group itself, `name`
            // will be the segment after the prefix.
            let mut command = match conf.commands.get_by_name(&*name) {
                Some(command) => command,
                None => {
                    return Err(Error::Dispatch(DispatchError::InvalidCommandName(
                        name.into_owned(),
                    )))
                }
            };

            let group = match group {
                Some(group) if group.commands.contains(&command.id) => group,
                Some(group) => {
                    return Err(Error::Dispatch(DispatchError::InvalidCommand(
                        group.id, command.id,
                    )))
                }
                None => conf
                    .top_level_groups
                    .iter()
                    .find(|g| g.commands.contains(&command.id))
                    .expect("command does not belong to any group"),
            };

            // Regardless whether we found a group (and its subgroups) or not,
            // `args` will be a substring of the message after the command.
            let mut args = segments.src;

            while let Some(name) = segments.next() {
                if let Some((id, aggr)) = conf.commands.get_pair(&*name) {
                    if conf.blocked_entities.commands.contains(&id) {
                        return Err(Error::Dispatch(DispatchError::BlockedCommand(id)));
                    }

                    if command.subcommands.contains(&id) {
                        command = aggr;
                        args = segments.src;
                        continue;
                    }
                }

                break;
            }

            break 'block (
                command.function,
                group.id,
                command.id,
                name.into_owned(),
                prefix.to_string(),
                args.to_string(),
            );
        };

        let ctx = Context {
            data: Arc::clone(&self.data),
            conf: Arc::clone(&self.conf),
            serenity_ctx: ctx,
            group_id,
            command_id,
            command_name,
            prefix,
            args,
        };

        hook(ctx, msg, func).await.map_err(Error::User)
    }
}
