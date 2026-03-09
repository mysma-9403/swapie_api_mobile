use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use std::str::FromStr;

// ── Enums ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum GenreType {
    Book,
    BoardGame,
}

impl fmt::Display for GenreType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenreType::Book => write!(f, "book"),
            GenreType::BoardGame => write!(f, "board_game"),
        }
    }
}

impl FromStr for GenreType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "book" => Ok(GenreType::Book),
            "board_game" => Ok(GenreType::BoardGame),
            _ => Err(format!("Invalid GenreType: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Genre {
    pub id: i64,
    pub name: String,
    pub slug: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub genre_type: GenreType,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
