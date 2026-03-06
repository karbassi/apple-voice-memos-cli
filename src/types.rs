use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Recording {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub duration: f64,
    pub date: chrono::DateTime<chrono::Local>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct State {
    pub processed: HashMap<String, ProcessedEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProcessedEntry {
    pub date: String,
    pub title: String,
    pub method: String,
    pub words: usize,
    pub output: Option<String>,
}
