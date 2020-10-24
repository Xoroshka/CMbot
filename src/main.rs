use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::env;
use std::error::Error;
use storage::DataBase;
use telegram_types::bot::methods;
use telegram_types::bot::types;
use warp::http::StatusCode;
use warp::{reject, Filter, Rejection, Reply};

pub(crate) mod chat;
pub(crate) mod graph_ql;
pub(crate) mod old_storage;
mod storage;

#[tokio::main]
async fn main() {
    let token = env::var("TG_BOT_TOKEN").expect("env var TG_BOT_TOKEN not set");
    let db = DataBase::open();
    let hello = warp::post()
        .and(warp::path(token))
        .and(warp::body::json())
        .and(with_db(db))
        .map(|update: types::Update, db: storage::DataBase| {
            // eprintln!("Get Update");
            match update.content {
                types::UpdateContent::Message(msg) => {
                    let answ = chat::UpdateReply {
                        method: chat::ApiMethod::SendMessage,
                        args: methods::SendMessage::new(
                            methods::ChatTarget::id(msg.chat.id.0),
                            "Hello",
                        ),
                    };
                    let pr = old_storage::Proceeds {
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

fn with_db(
    db: storage::DataBase,
) -> impl Filter<Extract = (storage::DataBase,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

// This function receives a `Rejection` and tries to return a custom
// value, otherwise simply passes the rejection along.
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "NOT_FOUND";
    // } else if let Some(DivideByZero) = err.find() {
    //     code = StatusCode::BAD_REQUEST;
    //     message = "DIVIDE_BY_ZERO";
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        // This error happens if the body could not be deserialized correctly
        // We can use the cause to analyze the error and customize the error message
        message = match e.source() {
            Some(cause) => {
                if cause.to_string().contains("denom") {
                    "FIELD_ERROR: denom"
                } else {
                    "BAD_REQUEST"
                }
            }
            None => "BAD_REQUEST",
        };
        code = StatusCode::BAD_REQUEST;
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        // We can handle a specific error, here METHOD_NOT_ALLOWED,
        // and render it however we want
        code = StatusCode::METHOD_NOT_ALLOWED;
        message = "METHOD_NOT_ALLOWED";
    } else {
        // We should have expected this... Just log and say its a 500
        eprintln!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "UNHANDLED_REJECTION";
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message: message.into(),
    });

    Ok(warp::reply::with_status(json, code))
}
