use serde::{Deserialize, Serialize};
use telegram_types::bot::methods;
use telegram_types::bot::types;
use crate::storage::{self, DataBase};
use std::future::Future;
use core::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ApiMethod {
    SendMessage,
}

#[derive(Serialize, Debug)]
pub(crate) struct UpdateReply<T: Serialize> {
    pub method: ApiMethod,
    #[serde(flatten)]
    pub json: T,
}

#[derive(Debug)]
pub enum HandleError {
    NotMessage,
    NoSender,
    DeactiveUser,
}

impl warp::reject::Reject for HandleError {}

// async fn handler(update: types::Update, db: DataBase) -> Result<impl warp::Reply, warp::Rejection> {
//     match update.content {
//         types::UpdateContent::Message(msg) => Ok(handle_message(msg, db.clone()).await),
//         _ => Err(warp::reject::custom(HandleError::NotMessage))
//     }
// }

// async fn handle_message(msg: types::Message, db:DataBase) -> Result<impl warp::Reply, warp::Rejection> {
//     if msg.from.is_none() {
//         // return warp::reply::json(&methods::SendMessage::new(methods::ChatTarget::id(msg.chat.id.0), "Использование бота не разрешено в каналах"));
//         return Err(warp::reject::custom(HandleError::NoSender))
//     };
//     let step = db.get_step(msg.from.unwrap().id.0).await;
//     match step {
//         storage::ChatStep::Deactive => Err(warp::reject::custom(HandleError::DeactiveUser)),
//     }
// }
        

