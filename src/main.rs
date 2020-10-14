use serde::{Deserialize, Serialize};
use std::env;
use storage::DataBase;
use telegram_types::bot::methods;
use telegram_types::bot::types;
use warp::Filter;

pub(crate) mod tg_api;
pub(crate) mod graph_ql;
pub(crate) mod storage;

#[tokio::main]
async fn main() {
    let token = env::var("TG_BOT_TOKEN").expect("env var TG_BOT_TOKEN not set");
    let db = DataBase::custom_init();
    let hello = warp::post()
        .and(warp::path(token))
        .and(warp::body::json())
        .and(with_db(db))
        .map(|update: types::Update, db: storage::DataBase| {
            // eprintln!("Get Update");
            match update.content {
                types::UpdateContent::Message(msg) => {
                    let answ = tg_api::UpdateReply {
                        method: tg_api::ApiMethod::SendMessage,
                        json: methods::SendMessage::new(
                            methods::ChatTarget::id(msg.chat.id.0),
                            "Hello",
                        ),
                    };
                    // eprintln!("Send msg");
                    // eprintln!("MSG:\n{}", serde_json::to_string(&answ).unwrap());
                    let pr = storage::Proceeds {
                        id: 0,
                        amount: 10000,
                        date: chrono::Local::now(),
                        post_date: chrono::Local::now(),
                        corner_id: 1,
                        user_id: 1,
                        comment: None,
                    };
                    warp::reply::json(&answ)
                }
                _ => {
                    let err = methods::ApiError {
                        error_code: 1,
                        description: "Error".to_owned(),
                        parameters: None,
                    };
                    warp::reply::json(&err)
                }
            }
        });

    warp::serve(hello)
        .tls()
        .cert_path("YOURPUBLIC.pem")
        .key_path("YOURPRIVATE.key")
        .run(([10, 0, 0, 10], 8443))
        .await;
}

fn with_db(db: storage::DataBase) -> impl Filter<Extract = (storage::DataBase,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}
