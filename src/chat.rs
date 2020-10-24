use crate::storage::{self, DataBase};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::future::Future;
use telegram_types::bot::methods;
use telegram_types::bot::types;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ApiMethod {
    SendMessage,
}

#[derive(Serialize, Debug)]
pub(crate) struct UpdateReply<T: Serialize> {
    pub method: ApiMethod,
    #[serde(flatten)]
    pub args: T,
}

#[derive(Debug)]
pub enum HandleError {
    NotMessage,
}
impl warp::reject::Reject for HandleError {}

fn main_handler(db: DataBase, update: types::Update) -> Result<warp::reply::Json, warp::Rejection> {
    let msg = if let types::UpdateContent::Message(m) = update.content {
        m
    } else {
        return Err(warp::reject::custom(HandleError::NotMessage));
    };

    let name: String = match msg.chat.kind {
        types::ChatType::Private {
            username: _,
            first_name: s,
            last_name: _,
        } => s,
        _ => return Ok(leave_chat(msg.chat.id.0)),
    };
    let chat_id = msg.chat.id.0;
    Ok(warp::reply::json(&match db.get_chat(chat_id) {
        None if msg.text.is_none() => send_msg(chat_id, GUEST_MSG),
        None => match db.register(msg.text.unwrap(), name) {
            false => send_msg(chat_id, FAIL_CODE),
            true => send_msg(chat_id, HELP),
        },

        Some(chat) if !chat.is_active => send_msg(chat_id, INACTIVE),
        Some(_) if msg.text.is_none() => send_msg(chat_id, HELP),
        Some(chat) => com_handler(msg.text.unwrap(), chat, db.clone()),
    }))
}

fn send_msg(chat_id: i64, text: &'static str) -> methods::SendMessage<'static> {
    methods::SendMessage::new(methods::ChatTarget::id(chat_id), text)
}

const GUEST_MSG: &'static str = "Ведите код приглашения";
const FAIL_CODE: &'static str = "Код не найден.
    \nВозможно у вас опечатка, либо срок действия кода истек.
    \nПроверьте правильность написания и попробуйте еще раз";
const INACTIVE: &'static str = "Ваш профиль был заблокирован администратором";
const HELP: &'static str = "Помощь";

fn leave_chat(chat_id: i64) -> warp::reply::Json {
    warp::reply::json(&serde_json::json!({
        "method": "leaveChat",
        "chat_id": chat_id
    }))
}

fn com_handler(com: String, chat: storage::Chat, db: DataBase) -> methods::SendMessage<'static> {
    todo!();
}
