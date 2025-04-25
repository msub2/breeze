use std::sync::LazyLock;

use native_tls::Identity;
use rusqlite::{params, Connection, Error, Result};

use crate::profile::Profile;

const DB: LazyLock<Connection> = LazyLock::new(|| {
    let db = Connection::open("breeze.db").unwrap();
    match db.execute("CREATE TABLE IF NOT EXISTS profiles (name TEXT PRIMARY KEY, cert TEXT, key TEXT, active BOOLEAN)", ()) {
    Ok(_) => (),
    Err(e) => panic!("Failed to create table: {}", e),
  }
    db
});

struct ProfileEntry {
    name: String,
    cert: String,
    key: String,
    active: bool,
}

pub fn new_profile(name: String, cert: String, key: String) -> Result<(), Error> {
    let count =
        match DB.query_row::<u32, _, _>("SELECT COUNT(*) FROM profiles;", [], |row| row.get(0)) {
            Ok(count) => count,
            Err(e) => return Err(e),
        };

    match DB.execute(
        "INSERT INTO profiles (name, cert, key, active) VALUES (?, ?, ?, ?);",
        (&name, &cert, &key, count == 0),
    ) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn get_default_profile() -> Result<Profile, Error> {
    let profile = DB.query_row::<ProfileEntry, _, _>(
        "SELECT * FROM profiles WHERE active = 1;",
        (),
        |row| {
            Ok(ProfileEntry {
                name: row.get(0)?,
                cert: row.get(1)?,
                key: row.get(2)?,
                active: row.get(3)?,
            })
        },
    )?;

    let identity = Identity::from_pkcs8(profile.cert.as_bytes(), profile.key.as_bytes()).unwrap();
    Ok(Profile {
        name: profile.name,
        identity,
        active: profile.active,
    })
}

pub fn get_all_profiles() -> Result<Vec<Profile>, Error> {
    let db = DB;
    let mut profiles = Vec::new();
    let mut stmt = db.prepare("SELECT * FROM profiles;")?;
    let profile_rows = stmt.query_map(params![], |row| {
        Ok(ProfileEntry {
            name: row.get(0)?,
            cert: row.get(1)?,
            key: row.get(2)?,
            active: row.get(3)?,
        })
    })?;
    for row in profile_rows {
        let profile = row?;
        let identity =
            Identity::from_pkcs8(profile.cert.as_bytes(), profile.key.as_bytes()).unwrap();
        profiles.push(Profile {
            name: profile.name,
            identity,
            active: profile.active,
        });
    }
    Ok(profiles)
}

pub fn set_active_profile(name: String) -> Result<(), Error> {
    DB.execute(
        "UPDATE profiles SET active = (CASE WHEN name = ? THEN true ELSE false END);",
        [name],
    )?;
    Ok(())
}
