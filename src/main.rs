mod simple_storage;
use teloxide::prelude::*;
use config::*;
use std::sync::{Arc, RwLock};
use std::collections::{HashSet, HashMap};
use teloxide::types::ChatId;
use crate::simple_storage::SimpleStorage;
use std::fs::OpenOptions;

#[tokio::main]
async fn main() {
    let mut config = Config::default();
    config
        .merge(config::File::with_name("config")).unwrap();

    let listening_clients_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open("listening_clients.txt")
        .unwrap();
    let listening_clients = SimpleStorage::new(listening_clients_file);

    let ignored_clients_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open("ignored_clients.txt")
        .unwrap();
    let ignored_clients = SimpleStorage::new(ignored_clients_file);

    start_bot(BotConfig {
        token: config.get_str("token").unwrap(),
        start_message: config.get_str("start_message").unwrap(),
        command_not_allowed: config.get_str("command_not_allowed").unwrap(),
        add_ignore: config.get_str("add_ignore").unwrap(),
        remove_ignore: config.get_str("remove_ignore").unwrap(),
        error_ignore: config.get_str("error_ignore").unwrap(),
        add_listener: config.get_str("add_listener").unwrap(),
        remove_listener: config.get_str("remove_listener").unwrap(),
        error_listener: config.get_str("error_listener").unwrap(),
        proxy_activate_code: config.get_str("proxy_activate_code").unwrap(),
        message_not_text_error: config.get_str("message_not_text_error").ok(),
        answer_after_message: config.get_str("answer_after_message").ok(),
        answer_after_message_ignored: config.get_str("answer_after_message_ignored").ok(),
        spam_control: config.get_table("spam_control").ok().map(|config| {
            BotSpamControlConfig {
                delay: config.get("delay").unwrap().clone().into_int().unwrap(),
                delayed_message: config.get("delayed_message").unwrap().clone().into_str().unwrap()
            }
        })
    }, listening_clients.clone(), ignored_clients.clone()).await;
}

#[derive(Debug, Clone)]
struct BotConfig {
    token: String,
    start_message: String,
    command_not_allowed: String,
    add_ignore: String,
    remove_ignore: String,
    error_ignore: String,
    add_listener: String,
    remove_listener: String,
    error_listener: String,
    proxy_activate_code: String,
    message_not_text_error: Option<String>,
    answer_after_message: Option<String>,
    answer_after_message_ignored: Option<String>,
    spam_control: Option<BotSpamControlConfig>
}

#[derive(Debug, Clone)]
struct BotSpamControlConfig {
    delay: i64,
    delayed_message: String
}

#[derive(Debug, Clone)]
struct IdsSet(HashSet<i64>);

async fn start_bot(
    config: BotConfig,
    listening_clients: SimpleStorage<IdsSet>,
    ignored_clients: SimpleStorage<IdsSet>
) {
    let config = Arc::new(config);
    let _ = listening_clients.sync_mem_from_file();
    let _ = ignored_clients.sync_mem_from_file();
    let clients_message_date: Arc<RwLock<HashMap<i64, i32>>> = Arc::new(Default::default());

    teloxide::enable_logging!();
    log::info!("Starting proxy-telegram-bot...");

    let bot = Bot::builder()
        .token(config.token.clone())
        .build();

    teloxide::repl(bot, move |message| {
        let config = config.clone();
        let listening_clients = listening_clients.clone();
        let ignored_clients = ignored_clients.clone();
        let clients_message_date = clients_message_date.clone();
        async move {
            let chat_id = message.update.chat.id;
            match config.spam_control.clone() {
                Some(spam_control) => {
                    let last_message_date = match clients_message_date.read() {
                        Ok(clients_message_date) => clients_message_date.get(&chat_id).cloned(),
                        Err(_) => None
                    };
                    match last_message_date {
                        Some(last_message_date) => {
                            let delay = spam_control.delay as i32;
                            if (last_message_date + delay) >= message.update.date {
                                message.answer_str(spam_control.delayed_message).await?;
                                return ResponseResult::<()>::Ok(());
                            }
                        },
                        None => {}
                    }
                },
                None => {}
            }
            let claim_message_date = || match clients_message_date.write() {
                Ok(mut clients_message_date) => {
                    clients_message_date.insert(chat_id, message.update.date);
                },
                Err(_) => {}
            };
            let is_listening_client = match listening_clients.mem_storage() {
                Ok(listening_clients) => listening_clients.0.contains(&chat_id),
                Err(_) => false
            };
            let is_ignored_client = match ignored_clients.mem_storage() {
                Ok(ignored_clients) => ignored_clients.0.contains(&chat_id),
                Err(_) => false
            };
            match message.update.text_owned() {
                Some(msg) => if msg.strip_prefix("/start").is_some() {
                    message.answer_str(config.start_message.clone()).await?;
                } else if let Some(after_ignore) = msg.strip_prefix("/ignore") {
                    if is_listening_client {
                        let answer = match after_ignore.trim().parse::<i64>() {
                            Ok(id) => match ignored_clients.mutable_mem_storage() {
                                Ok(mut ignored) => if ignored.0.contains(&id) {
                                    ignored.0.remove(&id);
                                    config.remove_ignore.clone()
                                } else {
                                    ignored.0.insert(id.clone());
                                    config.add_ignore.clone()
                                },
                                Err(_) => config.error_ignore.clone()
                            },
                            Err(_) => config.error_ignore.clone()
                        };
                        let _ = ignored_clients.sync_file_from_mem();
                        message.answer_str(answer).await?;
                    } else {
                        message.answer_str(config.command_not_allowed.clone()).await?;
                        claim_message_date();
                    }
                } else if let Some(after_listening) = msg.strip_prefix("/listening") {
                    if after_listening.trim() == config.proxy_activate_code {
                        let answer: String = match listening_clients.mutable_mem_storage() {
                            Ok(mut listeners) => if listeners.0.contains(&chat_id) {
                                listeners.0.remove(&chat_id);
                                config.add_listener.clone()
                            } else {
                                listeners.0.insert(chat_id.clone());
                                config.remove_listener.clone()
                            },
                            Err(_) => config.error_listener.clone()
                        };
                        let _ = listening_clients.sync_file_from_mem();
                        message.answer_str(answer).await?;
                    } else {
                        message.answer_str(config.command_not_allowed.clone()).await?;
                        claim_message_date();
                    }
                } else if !is_ignored_client {
                    let listeners = match listening_clients.mem_storage() {
                        Ok(listeners) => Some(listeners.0.clone()),
                        Err(_) => None
                    };
                    match listeners {
                        Some(listeners) => for listener in listeners {
                            if chat_id == listener {
                                continue;
                            }
                            let chat_id = ChatId::Id(listener);
                            let msg = match message.update.from() {
                                Some(user) => format!(
                                    "{} {} ({}) (Chat ID: {})\n\n{}",
                                    user.first_name.clone(),
                                    user.last_name.clone().unwrap_or("-".to_string()),
                                    user.username.clone().unwrap_or(user.id.clone().to_string()),
                                    message.update.chat.id.clone(),
                                    msg.clone()
                                ),
                                None => format!(
                                    "(Chat ID: {})\n\n{}",
                                    message.update.chat.id.clone(),
                                    msg.clone()
                                )
                            };
                            let _ = message.bot.send_message(chat_id.clone(), msg).send().await;
                        },
                        None => {}
                    }
                    match config.answer_after_message.clone() {
                        Some(answer_after_message) => {
                            message.answer_str(answer_after_message).await?;
                        },
                        None => {}
                    }
                    claim_message_date();
                } else {
                    match config.answer_after_message_ignored.clone() {
                        Some(answer_after_message_ignored) => {
                            message.answer_str(answer_after_message_ignored).await?;
                        },
                        None => {}
                    }
                    claim_message_date();
                },
                None => {
                    match config.message_not_text_error.clone() {
                        Some(message_not_text_error) => {
                            message.answer_str(message_not_text_error).await?;
                        },
                        None => {}
                    }
                    claim_message_date();
                }
            }
            ResponseResult::<()>::Ok(())
        }
    }).await;
}

impl Default for IdsSet {
    fn default() -> Self {
        IdsSet(HashSet::new())
    }
}

impl From<String> for IdsSet {
    fn from(string: String) -> Self {
        let lines = string.lines();
        let set = {
            let mut set = HashSet::<i64>::new();
            for line in lines {
                if let Ok(id) = line.to_owned().parse::<i64>() {
                    set.insert(id);
                }
            }
            set
        };
        IdsSet(set)
    }
}

impl Into<String> for IdsSet {
    fn into(self) -> String {
        let mut string = String::new();
        for id in self.0.iter() {
            string += &*(id.to_string() + "\n");
        }
        string
    }
}