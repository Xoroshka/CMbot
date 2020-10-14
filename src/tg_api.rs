use serde::{Deserialize, Serialize};
use telegram_types::bot::methods;
use telegram_types::bot::types;
use crate::storage::DataBase;

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



