use teloxide::prelude::*;
use config::*;
use std::sync::{Arc, RwLock};
use std::collections::{HashSet, HashMap};
use teloxide::types::ChatId;

#[tokio::main]
async fn main() {
    let mut config = Config::default();
    config
        .merge(config::File::with_name("config")).unwrap();


    start_bot(BotConfig {
        token: config.get_str("token").unwrap(),
        start_message: config.get_str("start_message").unwrap(),
        add_listener: config.get_str("add_listener").unwrap(),
        remove_listener: config.get_str("remove_listener").unwrap(),
        error_listener: config.get_str("error_listener").unwrap(),
        proxy_activate_code: config.get_str("proxy_activate_code").unwrap(),
        message_not_text_error: config.get_str("message_not_text_error").ok(),
        answer_after_message: config.get_str("answer_after_message").ok(),
        spam_control: config.get_table("spam_control").ok().map(|config| {
            BotSpamControlConfig {
                delay: config.get("delay").unwrap().clone().into_int().unwrap(),
                delayed_message: config.get("delayed_message").unwrap().clone().into_str().unwrap()
            }
        })
    }).await;
}

#[derive(Debug, Clone)]
struct BotConfig {
    token: String,
    start_message: String,
    add_listener: String,
    remove_listener: String,
    error_listener: String,
    proxy_activate_code: String,
    message_not_text_error: Option<String>,
    answer_after_message: Option<String>,
    spam_control: Option<BotSpamControlConfig>
}

#[derive(Debug, Clone)]
struct BotSpamControlConfig {
    delay: i64,
    delayed_message: String
}

async fn start_bot(config: BotConfig) {
    let config = Arc::new(config);
    let listening_clients: Arc<RwLock<HashSet<i64>>> = Arc::new(Default::default());
    let clients_message_date: Arc<RwLock<HashMap<i64, i32>>> = Arc::new(Default::default());

    teloxide::enable_logging!();
    log::info!("Starting proxy-telegram-bot...");

    let bot = Bot::builder()
        .token(config.token.clone())
        .build();

    teloxide::repl(bot, move |message| {
        let config = config.clone();
        let listening_clients = listening_clients.clone();
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
            match message.update.text_owned() {
                Some(msg) => if msg == String::from("/start") {
                    message.answer_str(config.start_message.clone()).await?;
                } else if msg == config.proxy_activate_code {
                    let answer: String = match listening_clients.write() {
                        Ok(mut listeners) => if listeners.contains(&chat_id) {
                            listeners.remove(&chat_id);
                            config.add_listener.clone()
                        } else {
                            listeners.insert(chat_id);
                            config.remove_listener.clone()
                        },
                        Err(_) => config.error_listener.clone()
                    };
                    message.answer_str(answer).await?;
                } else {
                    let listeners = match listening_clients.read() {
                        Ok(listeners) => Some(listeners.clone()),
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
                                    "{} {} ({})\n\n{}",
                                    user.first_name.clone(),
                                    user.last_name.clone().unwrap_or("-".to_string()),
                                    user.username.clone().unwrap_or(user.id.clone().to_string()),
                                    msg.clone()
                                ),
                                None => format!(
                                    "({})\n\n{}",
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
                    match clients_message_date.write() {
                        Ok(mut clients_message_date) => {
                            clients_message_date.insert(chat_id, message.update.date);
                        },
                        Err(_) => {}
                    };
                },
                None => {
                    match config.message_not_text_error.clone() {
                        Some(message_not_text_error) => {
                            message.answer_str(message_not_text_error).await?;
                        },
                        None => {}
                    }
                }
            }
            ResponseResult::<()>::Ok(())
        }
    }).await;
}