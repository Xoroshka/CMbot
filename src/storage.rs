use chrono::{Date, DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone)]
pub struct DataBase {
    sled: sled::Db,
}


impl DataBase {
    pub fn open() -> Self {
        let sled = sled::Config::new()
            .path("database".to_owned())
            .cache_capacity(250_000_000)
            .mode(sled::Mode::HighThroughput)
            .open()
            .map_err(|e| {
                eprintln!("ERROR: Can't open sled database");
                e
            })
            .unwrap();
        sled.iter().count();
        for name in sled.tree_names() {
            sled.open_tree(name).unwrap().iter().count();
        }
        DataBase { sled }
    }

    pub fn tree(&self, t: Tree) -> sled::Result<sled::Tree> {
        self.sled.open_tree([t as u8])
    }

    
}

trait BinVals 
    where Self: Serialize + DeserializeOwned {
    fn into_val(&self) -> sled::IVec {
        bincode::config()
            .big_endian()
            .serialize(self)
            .unwrap()
            .into()
    }

    fn from_val(vec: sled::IVec) -> Self {
        bincode::config().big_endian().deserialize(&vec).unwrap()
    }
}

pub enum Tree {
    Revenues,
    Chats,
    Corners,
    Stats,
    Invites,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Revenue {
    pub corner_id: u32,
    pub date: u32,
    pub user_id: u32,
    pub amount: u32,
    pub post_datetime: u32,
}

impl BinVals for Revenue {}

impl Revenue {
    fn key(date: u32, corner_id: u32) -> sled::IVec{
        let mut key: [u8; 8] = [0u8; 8];
        let mut v: [u8; 4] = date.to_be_bytes();
        key[0] = v[0];
        key[1] = v[1];
        key[2] = v[2];
        key[3] = v[3];
        v = corner_id.to_be_bytes();
        key[4] = v[0];
        key[5] = v[1];
        key[6] = v[2];
        key[7] = v[3];
        (&key).into()
    }

    fn into_key(&self) -> sled::IVec { 
        Self::key(self.date, self.corner_id)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    corner_id: u32,
    step: ChatStep,
    user_name: String,
    is_active: bool,
}

impl BinVals for Chat {}

impl Chat {
    fn key(chat_id: u64) -> sled::IVec {
        (&chat_id.to_be_bytes()).into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ChatStep {
    Start,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Corner {
    id: u32,
    name: String,
    tag: Option<String>,
}

impl BinVals for Corner {}

impl Corner {
    fn key(chat_id: u32) -> sled::IVec {
        (&chat_id.to_be_bytes()).into()
    }

    fn into_key(&self) -> sled::IVec {
        Self::key(self.id)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InviteCode {
    code: String,
    corner_id: u32,
    admin_id: u32,
    gen_date: u32,
}

impl BinVals for InviteCode {}

impl InviteCode {
    fn key(code: &str) -> sled::IVec {
        code.as_bytes().into()
    }

    fn into_key(&self) -> sled::IVec {
        Self::key(self.code.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use std::time::{Duration, SystemTime};

    #[test]
    fn open_sled() {
        DataBase::open();
    }

    #[test]
    fn insert_get() {
        let db = DataBase::open();
        let tree = db.tree(Tree::Revenues).unwrap();
        let mut rng = rand::thread_rng();
        // let mut start = SystemTime::now();
        // let mut time = SystemTime::now();
        let rev = Revenue {
            // corner_id: rng.next_u32(),
            corner_id: rng.next_u32(),
            date: rng.next_u32(),
            user_id: rng.next_u32(),
            amount: rng.next_u32(),
            post_datetime: rng.next_u32(),
        };
        let key = rev.into_key();
        tree.insert(&key, rev.into_val()).unwrap();
        assert_eq!(Revenue::from_val(tree.get(&key).unwrap().unwrap()), rev);
        // eprintln!("Total revenues: {}", tree.iter().count());
        // eprintln!("Total time {} secs", start.elapsed().unwrap().as_secs())
        // dbg!(tree
        //     .iter()
        //     .collect::<sled::Result<Vec<(sled::IVec, sled::IVec)>>>()
        //     .unwrap());
    }
}
