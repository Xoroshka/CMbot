use chrono::{DateTime, Local, TimeZone};
use rand::Rng;
use rusqlite::{
    params,
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
    Connection, OptionalExtension, NO_PARAMS,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

const INVITE_LEN: usize = 8;

trait ConnectionExt {
    fn invite_code_exist(&self, code: &str) -> anyhow::Result<bool>;
    fn use_invite_code(&self, code: &str) -> anyhow::Result<RegisterResult>;
}

impl ConnectionExt for Connection {
    fn invite_code_exist(&self, code: &str) -> anyhow::Result<bool> {
        let mut statement = self.prepare("SELECT * FROM invite_code WHERE code=?1")?;
        let res = statement.exists(params![code])?;
        Ok(res)
    }

    fn use_invite_code(&self, code: &str) -> anyhow::Result<RegisterResult> {
        let mut stmt = self.prepare("SELECT * FROM invite_code WHERE code=?1")?;
        let code_from_db = stmt
            .query_row(rusqlite::params![code], |row| {
                Ok(InviteCode {
                    code: row.get(0)?,
                    corner_id: row.get(1)?,
                    admin_id: row.get(2)?,
                    gen_date: Local.timestamp(row.get(3)?, 0),
                    used: row.get(4)?,
                })
            })
            .optional()?;
        if code_from_db.is_none() {
            return Ok(RegisterResult::InviteNotFound);
        }
        match code_from_db.unwrap() {
            cd if cd.used == true => return Ok(RegisterResult::InviteUsed),
            cd if chrono::Local::now()
                .signed_duration_since(cd.gen_date)
                .num_days()
                > 1 =>
            {
                return Ok(RegisterResult::InviteExpired)
            }
            cd => {
                self.execute(
                    "UPDATE invite_code SET used = 1 WHERE code=?1",
                    rusqlite::params![cd.code],
                )?;
                return Ok(RegisterResult::Succes(cd.corner_id));
            }
        }
    }
}

#[derive(Clone)]
pub struct DataBase {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl DataBase {
    pub fn custom_init() -> Self {
        let db_file = "./database.db3";
        let conn = Connection::open(&db_file).expect(&format!("Can't open db3 file: {}", &db_file));

        conn.execute(
            "CREATE TABLE IF NOT EXISTS proceeds (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                amount INTEGER NOT NULL CHECK (amount >=0),
                date INTEGER NOT NULL,
                post_date INTEGER NOT NULL,
                corner_id INTEGER REFERENCES corner,
                user_id INTEGER REFERENCES user,
                comment TEXT
            )",
            NO_PARAMS,
        )
        .expect("Can't check/create proceeds table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS user (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tg_id INTEGER UNIQUE NOT NULL,
                name TEXT NOT NULL,
                corner_id INTEGER REFERENCES corner,
                is_active INTEGER NOT NULL,
                step INTEGER NOT NULL
            )",
            NO_PARAMS,
        )
        .expect("Can't check/create user table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS corner (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                shrt_name TEXT UNIQUE
            )",
            NO_PARAMS,
        )
        .expect("Can't check/create corner table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS admin (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                login TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                pswd_hash TEXT NOT NULL
            )",
            NO_PARAMS,
        )
        .expect("Can't check/create user table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS invite_code (
                code TEXT PRIMARY KEY,
                corner_id INTEGER NOT NULL,
                admin_id INTEGER NOT NULL,
                gen_date INTEGER NOT NULL,
                used INTEGER NOT NULL
            )",
            NO_PARAMS,
        )
        .expect("Can't check/create invite_code table");

        DataBase {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    pub async fn push_proceeds(&self, pr: Proceeds) -> anyhow::Result<usize> {
        let conn = self.conn.lock().await;
        let res: usize = conn.execute(
            "INSERT INTO proceeds (amount, date, post_date, corner_id, user_id, comment)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                pr.amount,
                pr.date.timestamp(),
                pr.post_date.timestamp(),
                pr.corner_id,
                pr.user_id,
                pr.comment
            ],
        )?;
        Ok(res)
    }

    pub async fn get_proceeds(&self) -> anyhow::Result<Vec<Proceeds>> {
        let conn = self.conn.lock().await;
        let mut statement = conn.prepare("SELECT * FROM proceeds")?;
        let res = serde_rusqlite::from_rows::<Proceeds>(statement.query(NO_PARAMS)?)
            .collect::<serde_rusqlite::error::Result<Vec<Proceeds>>>()?;
        Ok(res)
    }

    pub async fn get_new_invite_code(
        &self,
        corner_id: i32,
        admin_id: i32,
    ) -> anyhow::Result<String> {
        let conn = self.conn.lock().await;
        loop {
            let code = gen_code();
            if conn.invite_code_exist(code.as_str())? {
                continue;
            }
            conn.execute(
                "INSERT INTO invite_code (code, corner_id, admin_id, gen_date, used) 
            VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    code,
                    corner_id,
                    admin_id,
                    chrono::Local::now().timestamp(),
                    false
                ],
            )?;
            return Ok(code);
        }
    }

    pub async fn register_user(
        &self,
        code_and_name: &str,
        tg_id: i32,
    ) -> anyhow::Result<RegisterResult> {
        let conn = self.conn.lock().await;
        if code_and_name.len() < 15 {
            Ok(RegisterResult::TooShortName)
        } else {
            match conn.use_invite_code(&code_and_name[..INVITE_LEN])? {
                RegisterResult::Succes(corner_id) => {
                    conn.execute(
                        "INSERT INTO user (tg_id, name, corner_id, is_active, step) 
                        VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![tg_id, &code_and_name[INVITE_LEN + 1..], corner_id, true, 0],
                    )?;
                    Ok(RegisterResult::Succes(conn.last_insert_rowid() as i32))
                }
                invite_error => Ok(invite_error),
            }
        }
    }

    pub async fn get_user(&self, id: i32) -> anyhow::Result<User> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached("SELECT * FROM user WHERE id=?1")?;
        let user: User = stmt.query_row(&[id], User::from_row)?;
        Ok(user)
    }

    pub async fn get_users_by_corner(&self, corner_id: i32) -> anyhow::Result<Vec<User>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached("SELECT * FROM user WHERE corner_id=?1")?;
        let res: rusqlite::Result<Vec<User>> = stmt
            .query_and_then(params![corner_id], User::from_row)?
            .collect();
        Ok(res?)
    }

    pub async fn get_step(&self, tg_id: i64) -> anyhow::Result<ChatStep> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT is_active, step FROM user WHERE tg_id=?1")?;
        let step: ChatStep = stmt
            .query_row(rusqlite::params![tg_id], |row| {
                Ok(match row.get::<usize, bool>(0)? {
                    true => row.get::<usize, ChatStep>(1)?,
                    false => ChatStep::Deactive
                })
                
            })
            .optional()?.unwrap_or(ChatStep::NotRegister);
        Ok(step)
    }

    pub async fn deactive_user(&self, id: i32) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("UPDATE user SET is_active=0 WHERE id=?1", params![id])?;
        Ok(())
    }

    pub async fn active_user(&self, id: i32) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("UPDATE user SET is_active=1 WHERE id=?1", params![id])?;
        Ok(())
    }

    pub async fn set_name_for_user(&self, name: &str, id: i32) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("UPDATE user SET name=?1 WHERE id=?2", params![name, id])?;
        Ok(())
    }

    pub async fn del_user(&self, id: i32) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM user WHERE id=?1", params![id])?;
        Ok(())
    }

    pub async fn del_proceeds(&self, id: i32) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM proceeds WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn update_proceeds(&self) -> anyhow::Result<()> {
        todo!();
    }

    pub async fn push_corner(
        &self,
        name: &str,
        shrt_name: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO corner (name, shrt_name) VALUES (?1, ?2)",
            params![name, shrt_name],
        )?;
        Ok(())
    }

    pub async fn del_corner(&self, id: i32) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM corner WHERE id=?1", params![id])?;
        Ok(())
    }

    pub async fn get_corners(&self) -> anyhow::Result<Vec<Corner>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare_cached("SELECT * FROM corner")?;
        let res: rusqlite::Result<Vec<Corner>> =
            stmt.query_and_then(NO_PARAMS, Corner::from_row)?.collect();
        Ok(res?)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Proceeds {
    #[serde(skip_serializing_if = "i32_is_null")]
    pub id: i32,
    pub amount: i32,
    pub date: chrono::DateTime<chrono::Local>,
    pub post_date: chrono::DateTime<chrono::Local>,
    pub corner_id: i32,
    pub user_id: i32,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: i32,
    tg_id: i32,
    name: String,
    corner_id: i32,
    is_active: bool,
    step: ChatStep,
}

impl User {
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(User {
            id: row.get(0)?,
            tg_id: row.get(1)?,
            name: row.get(2)?,
            corner_id: row.get(3)?,
            is_active: row.get(4)?,
            step: row.get(5)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatStep {
    Start,
    NotRegister,
    Deactive
}

impl FromSql for ChatStep {
    fn column_result(value: ValueRef) -> FromSqlResult<Self>{
        match value {
            ValueRef::Integer(_) => Ok(ChatStep::Start),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Corner {
    #[serde(skip_serializing_if = "i32_is_null")]
    id: i32,
    name: String,
    shrt_name: Option<String>,
}

impl Corner {
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Corner {
            id: row.get(0)?,
            name: row.get(1)?,
            shrt_name: row.get(3)?,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InviteCode {
    code: String,
    corner_id: i32,
    admin_id: i32,
    gen_date: chrono::DateTime<chrono::Local>,
    used: bool,
}

#[derive(Debug)]
pub enum RegisterResult {
    Succes(i32),
    InviteExpired,
    InviteUsed,
    InviteNotFound,
    TooShortName,
}

fn i32_is_null(x: &i32) -> bool {
    *x == 0
}

fn gen_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    let mut rng = rand::thread_rng();

    let code: String = (0..INVITE_LEN)
        .map(|_| {
            let idx = rng.gen_range(0, CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rand_id() -> i32 {
        rand::thread_rng().gen_range(1325039, 9142134)
    }

    #[tokio::test]
    async fn full_invite_check() {
        let db = DataBase::custom_init();
        let code = db.get_new_invite_code(1, 1).await.unwrap();
        let conn = db.conn.lock().await;
        assert!(!conn.invite_code_exist("1234ABCD").unwrap());
        assert!(conn.invite_code_exist(code.as_str()).unwrap());
        let res = conn.use_invite_code(code.as_str()).unwrap();
        match res {
            RegisterResult::Succes(x) => assert_eq!(x, 1),
            _ => panic!(),
        }

        let res = conn.use_invite_code(code.as_str()).unwrap();
        match res {
            RegisterResult::InviteUsed => {}
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn register_check() {
        let tg_id: i32 = rand_id();
        let db = DataBase::custom_init();
        let mut code = db.get_new_invite_code(10, 1).await.unwrap();
        dbg!(code.clone());
        code.push_str(" Иванов Иван");
        let res = db.register_user(code.as_str(), tg_id).await.unwrap();
        let mut last_id: i32 = 0;
        if let RegisterResult::Succes(id) = res {
            last_id = id;
        }
        let user_out = db.get_user(last_id).await.unwrap();
        let user_in = User {
            id: last_id,
            tg_id,
            name: String::from("Иванов Иван"),
            corner_id: 10,
            is_active: true,
            step: ChatStep::Start
        };
        assert_eq!(user_in, user_out);
    }
}
