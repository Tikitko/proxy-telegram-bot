use teloxide::prelude::*;
use config::*;
use std::sync::{Arc, RwLock};
use std::collections::HashSet;
use teloxide::types::ChatId;

#[tokio::main]
async fn main() {
    let mut config = Config::default();
    config
        .merge(config::File::with_name("config")).unwrap();

    start_bot(BotConfig {
        token: config.get_str("token").unwrap(),
        start_message: config.get_str("start_message").unwrap(),
        proxy_activate_code: config.get_str("proxy_activate_code").unwrap()
    }).await;
}

struct BotConfig {
    token: String,
    start_message: String,
    proxy_activate_code: String
}

async fn start_bot(config: BotConfig) {
    let config = Arc::new(config);
    let listening_clients: Arc<RwLock<HashSet<i64>>> = Arc::new(Default::default());

    let bot = Bot::builder()
        .token(config.token.clone())
        .build();

    teloxide::repl(bot, move |message| {
        let config = config.clone();
        let listening_clients = listening_clients.clone();
        async move {
            let chat_id = message.update.chat.id;
            match message.update.text_owned() {
                Some(msg) => if msg == String::from("/start") {
                    message.answer_str(config.start_message.clone()).await?;
                } else if msg == config.proxy_activate_code {
                    let answer: String = match listening_clients.write() {
                        Ok(mut listeners) => if listeners.contains(&chat_id) {
                            listeners.remove(&chat_id);
                            String::from("You was removed from listeners!")
                        } else {
                            listeners.insert(chat_id.clone());
                            String::from("You was added to listeners!")
                        },
                        Err(err) => format!("Error {} on listeners storage!", err)
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
                            message.bot.send_message(chat_id, msg).send().await?;
                        },
                        None => {}
                    }
                },
                None => {}
            }
            ResponseResult::<()>::Ok(())
        }
    }).await;
}