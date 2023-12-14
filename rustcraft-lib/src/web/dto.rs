use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Player {
    pub username: String,
    pub uuid: String,
    pub position: Position,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub on_ground: bool,
}