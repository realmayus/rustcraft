use async_nbt::io::Flavor;
use async_nbt::NbtCompound;
use async_trait::async_trait;
use serde_json::Map;
use tokio::io::{AsyncRead, AsyncWrite};
use uuid::Uuid;

use rustcraft_derive::{ReadProt, SizedProt, WriteProt};

use crate::protocol_types::primitives::{SizedVec, VarInt};
use crate::protocol_types::traits::{ReadProt, SizedProt, WriteProt};

#[async_trait]
impl ReadProt for Uuid {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let mut buf: u128 = 0;
        buf |= u64::read(stream).await? as u128;
        buf <<= 8 * 8;
        buf |= u64::read(stream).await? as u128;
        Ok(Uuid::from_u128(buf))
    }
}

#[async_trait]
impl WriteProt for Uuid {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let buf = self.as_u128();
        let buf1 = (buf >> 8 * 8) as u64;
        let buf2 = buf as u64;
        buf1.write(stream).await?;
        buf2.write(stream).await?;
        Ok(())
    }
}

impl SizedProt for Uuid {
    fn prot_size(&self) -> usize {
        16
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub(crate) struct Position {
    pub(crate) x: i32,
    // actual size: 26 bits
    pub(crate) z: i32,
    // actual size: 26 bits
    pub(crate) y: i32, // actual size: 12 bits
}

impl Position {
    pub(crate) fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[async_trait]
impl WriteProt for Position {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let int = (((self.x & 0x3FFFFFF) as i64) << 38)
            | (((self.z & 0x3FFFFFF) as i64) << 12)
            | (self.y as i64 & 0xFFF);
        int.write(stream).await
    }
}

#[async_trait]
impl ReadProt for Position {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let int = i64::read(stream).await?;
        Ok(Self {
            x: (int >> 38) as i32,
            z: (int << 26 >> 38) as i32,
            y: (int << 52 >> 52) as i32,
        })
    }
}

impl SizedProt for Position {
    fn prot_size(&self) -> usize {
        8
    }
}

#[async_trait]
impl WriteProt for NbtCompound {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        async_nbt::io::write_nbt(stream, None, self, Flavor::Uncompressed)
            .await
            .or_else(|x| Err(format!("NBT error: {:?}", x)))?;
        Ok(())
    }
}

#[async_trait]
impl ReadProt for NbtCompound {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(async_nbt::io::read_nbt(stream, Flavor::Uncompressed, true)
            .await
            .or_else(|x| Err(format!("NBT error: {:?}", x)))?
            .0)
    }
}

impl SizedProt for NbtCompound {
    fn prot_size(&self) -> usize {
        async_nbt::io::size(self, Flavor::Uncompressed, None).unwrap()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Recipe {
    typ: String,
    id: String,
    data: RecipeKind,
}

#[derive(Debug, Clone)]
enum RecipeKind {
    CraftingShapeless {
        group: String,
        category: VarInt,
        ingredients: SizedVec<RecipeIngredient>,
        result: Slot,
    },
    CraftingShaped {
        width: VarInt,
        height: VarInt,
        group: String,
        category: VarInt,
        ingredients: SizedVec<RecipeIngredient>,
        result: Slot,
        show_notification: bool,
    },
    // crafting_special_{armordye, bookcloning, mapcloning, mapextending, firework_rocket, firework_star, firework_star_fade, repairitem, tippedarrow, bannerduplicate, shielddecoration, shulkerboxcoloring, suspiciousstew}, crafting_decorated_pot
    CraftingSpecial {
        category: VarInt,
    },
    // smelting, blasting, smoking, campfire_cooking
    Smelting {
        group: String,
        category: VarInt,
        ingredient: RecipeIngredient,
        result: Slot,
        experience: f32,
        cooking_time: VarInt,
    },
    Stonecutting {
        group: String,
        ingredient: RecipeIngredient,
        result: Slot,
    },
    SmithingTransform {
        template: RecipeIngredient,
        base: RecipeIngredient,
        addition: RecipeIngredient,
        result: Slot,
    },
    SmithingTrim {
        template: RecipeIngredient,
        base: RecipeIngredient,
        addition: RecipeIngredient,
    },
}

#[async_trait]
impl ReadProt for Recipe {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let typ = String::read(stream).await?;
        let id = String::read(stream).await?;
        Ok(match typ.as_str() {
            "minecraft:crafting_shapeless" => Self {
                typ,
                id,
                data: RecipeKind::CraftingShapeless {
                    group: String::read(stream).await?,
                    category: VarInt::read(stream).await?,
                    ingredients: SizedVec::read(stream).await?,
                    result: Slot::read(stream).await?,
                },
            },
            "minecraft:crafting_shaped" => Self {
                typ,
                id,
                data: RecipeKind::CraftingShaped {
                    width: VarInt::read(stream).await?,
                    height: VarInt::read(stream).await?,
                    group: String::read(stream).await?,
                    category: VarInt::read(stream).await?,
                    ingredients: SizedVec::read(stream).await?,
                    result: Slot::read(stream).await?,
                    show_notification: bool::read(stream).await?,
                },
            },
            "minecraft:crafting_special_armordye"
            | "minecraft:crafting_special_bookcloning"
            | "minecraft:crafting_special_mapcloning"
            | "minecraft:crafting_special_mapextending"
            | "minecraft:crafting_special_firework_rocket"
            | "minecraft:crafting_special_firework_star"
            | "minecraft:crafting_special_firework_star_fade"
            | "minecraft:crafting_special_repairitem"
            | "minecraft:crafting_special_tippedarrow"
            | "minecraft:crafting_special_bannerduplicate"
            | "minecraft:crafting_special_shielddecoration"
            | "minecraft:crafting_special_shulkerboxcoloring"
            | "minecraft:crafting_special_suspiciousstew"
            | "minecraft:crafting_decorated_pot" => Self {
                typ,
                id,
                data: RecipeKind::CraftingSpecial {
                    category: VarInt::read(stream).await?,
                },
            },
            "minecraft:smelting"
            | "minecraft:blasting"
            | "minecraft:smoking"
            | "minecraft:campfire_cooking" => Self {
                typ,
                id,
                data: RecipeKind::Smelting {
                    group: String::read(stream).await?,
                    category: VarInt::read(stream).await?,
                    ingredient: RecipeIngredient::read(stream).await?,
                    result: Slot::read(stream).await?,
                    experience: f32::read(stream).await?,
                    cooking_time: VarInt::read(stream).await?,
                },
            },
            "minecraft:stonecutting" => Self {
                typ,
                id,
                data: RecipeKind::Stonecutting {
                    group: String::read(stream).await?,
                    ingredient: RecipeIngredient::read(stream).await?,
                    result: Slot::read(stream).await?,
                },
            },
            "minecraft:smithing_transform" => Self {
                typ,
                id,
                data: RecipeKind::SmithingTransform {
                    template: RecipeIngredient::read(stream).await?,
                    base: RecipeIngredient::read(stream).await?,
                    addition: RecipeIngredient::read(stream).await?,
                    result: Slot::read(stream).await?,
                },
            },
            "minecraft:smithing_trim" => Self {
                typ,
                id,
                data: RecipeKind::SmithingTrim {
                    template: RecipeIngredient::read(stream).await?,
                    base: RecipeIngredient::read(stream).await?,
                    addition: RecipeIngredient::read(stream).await?,
                },
            },
            _ => return Err("Unknown recipe type".into()),
        })
    }
}

#[async_trait]
impl WriteProt for Recipe {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        self.typ.write(stream).await?;
        self.id.write(stream).await?;
        match &self.data {
            RecipeKind::CraftingShapeless {
                group,
                category,
                ingredients,
                result,
            } => {
                group.write(stream).await?;
                category.write(stream).await?;
                ingredients.write(stream).await?;
                result.write(stream).await?;
            }
            RecipeKind::CraftingShaped {
                width,
                height,
                group,
                category,
                ingredients,
                result,
                show_notification,
            } => {
                width.write(stream).await?;
                height.write(stream).await?;
                group.write(stream).await?;
                category.write(stream).await?;
                ingredients.write(stream).await?;
                result.write(stream).await?;
                show_notification.write(stream).await?;
            }
            RecipeKind::CraftingSpecial { category } => {
                category.write(stream).await?;
            }
            RecipeKind::Smelting {
                group,
                category,
                ingredient,
                result,
                experience,
                cooking_time,
            } => {
                group.write(stream).await?;
                category.write(stream).await?;
                ingredient.write(stream).await?;
                result.write(stream).await?;
                experience.write(stream).await?;
                cooking_time.write(stream).await?;
            }
            RecipeKind::Stonecutting {
                group,
                ingredient,
                result,
            } => {
                group.write(stream).await?;
                ingredient.write(stream).await?;
                result.write(stream).await?;
            }
            RecipeKind::SmithingTransform {
                template,
                base,
                addition,
                result,
            } => {
                template.write(stream).await?;
                base.write(stream).await?;
                addition.write(stream).await?;
                result.write(stream).await?;
            }
            RecipeKind::SmithingTrim {
                template,
                base,
                addition,
            } => {
                template.write(stream).await?;
                base.write(stream).await?;
                addition.write(stream).await?;
            }
        }
        Ok(())
    }
}

impl SizedProt for Recipe {
    fn prot_size(&self) -> usize {
        self.typ.prot_size()
            + self.id.prot_size()
            + match &self.data {
                RecipeKind::CraftingShapeless {
                    group,
                    category,
                    ingredients,
                    result,
                } => {
                    group.prot_size()
                        + category.prot_size()
                        + ingredients.prot_size()
                        + result.prot_size()
                }
                RecipeKind::CraftingShaped {
                    width,
                    height,
                    group,
                    category,
                    ingredients,
                    result,
                    show_notification,
                } => {
                    width.prot_size()
                        + height.prot_size()
                        + group.prot_size()
                        + category.prot_size()
                        + ingredients.prot_size()
                        + result.prot_size()
                        + show_notification.prot_size()
                }
                RecipeKind::CraftingSpecial { category } => category.prot_size(),
                RecipeKind::Smelting {
                    group,
                    category,
                    ingredient,
                    result,
                    experience,
                    cooking_time,
                } => {
                    group.prot_size()
                        + category.prot_size()
                        + ingredient.prot_size()
                        + result.prot_size()
                        + experience.prot_size()
                        + cooking_time.prot_size()
                }
                RecipeKind::Stonecutting {
                    group,
                    ingredient,
                    result,
                } => group.prot_size() + ingredient.prot_size() + result.prot_size(),
                RecipeKind::SmithingTransform {
                    template,
                    base,
                    addition,
                    result,
                } => {
                    template.prot_size()
                        + base.prot_size()
                        + addition.prot_size()
                        + result.prot_size()
                }
                RecipeKind::SmithingTrim {
                    template,
                    base,
                    addition,
                } => template.prot_size() + base.prot_size() + addition.prot_size(),
            }
    }
}

type RecipeIngredient = SizedVec<Slot>;

#[derive(SizedProt, WriteProt, Debug, Clone)]
struct Slot {
    present: bool,
    item_id: Option<VarInt>,
    item_count: Option<u8>,
    nbt: Option<NbtCompound>,
}

#[async_trait]
impl ReadProt for Slot {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let present = bool::read(stream).await?;
        let item_id = if present {
            Some(VarInt::read(stream).await?)
        } else {
            None
        };
        let item_count = if present {
            Some(u8::read(stream).await?)
        } else {
            None
        };
        let nbt = if present {
            Some(NbtCompound::read(stream).await?)
        } else {
            None
        };
        Ok(Self {
            present,
            item_id,
            item_count,
            nbt,
        })
    }
}

#[derive(SizedProt, WriteProt, ReadProt, Debug, Clone)]
pub(crate) struct TagGroup {
    typ: String, // minecraft:block, minecraft:item, minecraft:fluid, minecraft:entity_type, and minecraft:game_event
    tags: SizedVec<Tag>,
}

#[derive(SizedProt, WriteProt, ReadProt, Debug, Clone)]
pub(crate) struct Tag {
    name: String,
    types: SizedVec<VarInt>, // numeric IDs of the given type (block, item, etc.)
}

#[derive(ReadProt, WriteProt, SizedProt, Debug, Clone)]
pub(crate) struct BitSet(pub(crate) SizedVec<i64>);
impl BitSet {
    pub(crate) fn bit(&self, n: usize) -> bool {
        self.0.vec[n / 64] & (1i64 << (n % 64)) != 0
    }

    // untested
    pub(crate) fn set_bit(&mut self, n: usize, value: bool) {
        if value {
            self.0.vec[n / 64] |= 1i64 << (n % 64);
        } else {
            self.0.vec[n / 64] &= !(1i64 << (n % 64));
        }
    }
}

#[derive(SizedProt, ReadProt, WriteProt, Debug, Clone)]
pub(crate) struct BlockEntity {
    xz: u8,
    y: i16,
    typ: VarInt,
    data: NbtCompound,
}

#[derive(Debug, Clone)]
pub(crate) struct Chat {
    map: Map<String, serde_json::Value>,
}
impl Chat {
    pub(crate) fn new_text(text: String) -> Self {
        let mut map = serde_json::map::Map::new();
        map.insert("text".into(), serde_json::Value::String(text));
        Self { map }
    }

    pub(crate) fn with_extra(mut self, new_extra: Chat) -> Self {
        if self.map.contains_key("extra") {
            let extra = self.map.get_mut("extra").unwrap();
            if let serde_json::Value::Array(arr) = extra {
                arr.push(serde_json::Value::Object(new_extra.map));
            }
        } else {
            self.map.insert(
                "extra".into(),
                serde_json::Value::Array(vec![serde_json::Value::Object(new_extra.map)]),
            );
        }
        self
    }

    pub(crate) fn with_bold(mut self, bold: bool) -> Self {
        self.map
            .insert("bold".into(), serde_json::Value::Bool(bold));
        self
    }

    pub(crate) fn with_italic(mut self, italic: bool) -> Self {
        self.map
            .insert("italic".into(), serde_json::Value::Bool(italic));
        self
    }

    pub(crate) fn with_underlined(mut self, underlined: bool) -> Self {
        self.map
            .insert("underlined".into(), serde_json::Value::Bool(underlined));
        self
    }

    pub(crate) fn with_strikethrough(mut self, strikethrough: bool) -> Self {
        self.map.insert(
            "strikethrough".into(),
            serde_json::Value::Bool(strikethrough),
        );
        self
    }

    pub(crate) fn with_obfuscated(mut self, obfuscated: bool) -> Self {
        self.map
            .insert("obfuscated".into(), serde_json::Value::Bool(obfuscated));
        self
    }

    pub(crate) fn with_color(mut self, color: String) -> Self {
        self.map
            .insert("color".into(), serde_json::Value::String(color));
        self
    }
}

#[async_trait]
impl ReadProt for Chat {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String> {
        let json = String::read(stream).await?;
        let map = serde_json::from_str(&json).unwrap();
        Ok(Self { map })
    }
}

#[async_trait]
impl WriteProt for Chat {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let json = serde_json::to_string(&serde_json::Value::Object(self.map.clone())).unwrap();
        json.write(stream).await?;
        Ok(())
    }
}

impl SizedProt for Chat {
    fn prot_size(&self) -> usize {
        let json = serde_json::to_string(&serde_json::Value::Object(self.map.clone())).unwrap();
        json.prot_size()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

#[derive(Clone, Debug)]
pub(crate) enum WinGame {
    JustRespawn,
    RollCreditsAndRespawn,
}

#[derive(Clone, Debug)]
pub(crate) enum GameEvent {
    NoRespawnBlock,
    StartRain,
    EndRain,
    SetGameMode(GameMode),
    WinGame(WinGame),
    ArrowHit,
    SetRainLevel(f32),
    SetThunderLevel(f32),
    PufferfishSting,
    ElderGuardianAppearance,
    SetRespawnScreen(bool),
    SetLimitedCrafting(bool)
}

#[async_trait]
impl WriteProt for GameEvent {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        match self {
            GameEvent::NoRespawnBlock => {
                0u8.write(stream).await?;
                0f32.write(stream).await?;
            }
            GameEvent::StartRain => {
                1u8.write(stream).await?;
                0f32.write(stream).await?;
            }
            GameEvent::EndRain => {
                2u8.write(stream).await?;
                0f32.write(stream).await?;
            }
            GameEvent::SetGameMode(mode) => {
                3u8.write(stream).await?;
                let val = match mode {
                    GameMode::Survival => 0,
                    GameMode::Creative => 1,
                    GameMode::Adventure => 2,
                    GameMode::Spectator => 3
                };
                (val as f32).write(stream).await?;
            }
            GameEvent::WinGame(mode) => {
                4u8.write(stream).await?;
                let val = match mode {
                    WinGame::JustRespawn => 0,
                    WinGame::RollCreditsAndRespawn => 1
                };
                (val as f32).write(stream).await?;
            }
            GameEvent::ArrowHit => {
                6u8.write(stream).await?;
                0f32.write(stream).await?;
            }
            GameEvent::SetRainLevel(level) => {
                7u8.write(stream).await?;
                level.write(stream).await?;
            }
            GameEvent::SetThunderLevel(level) => {
                8u8.write(stream).await?;
                level.write(stream).await?;
            }
            GameEvent::PufferfishSting => {
                9u8.write(stream).await?;
                0f32.write(stream).await?;
            }
            GameEvent::ElderGuardianAppearance => {
                10u8.write(stream).await?;
                0f32.write(stream).await?;
            }
            GameEvent::SetRespawnScreen(mode) => {
                11u8.write(stream).await?;
                let val = match mode {
                    true => 1,
                    false => 0
                };
                (val as f32).write(stream).await?;
            }
            GameEvent::SetLimitedCrafting(mode) => {
                12u8.write(stream).await?;
                let val = match mode {
                    true => 1,
                    false => 0
                };
                (val as f32).write(stream).await?;
            }
        }
        Ok(())
    }
}

impl SizedProt for GameEvent {
    fn prot_size(&self) -> usize {
        1 + 4
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PosRotGround {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
    pub(crate) pitch: f64,
    pub(crate) yaw: f64,
    pub(crate) on_ground: bool,
}

impl From<PosRotGround> for Position {
    fn from(pos: PosRotGround) -> Self {
        Self {
            x: pos.x.floor() as i32,
            y: pos.y.floor() as i32,
            z: pos.z.floor() as i32,
        }
    }
}


#[derive(Debug, Copy, Clone)]
pub(crate) enum PlayerActions {
    StartDig,
    CancelDig,
    FinishDig,
    DropStack,
    DropItem,
    ShootArrowFinishEating,
    SwapHands,
}

#[async_trait]
impl ReadProt for PlayerActions {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String> where Self: Sized {
        let action = VarInt::read(stream).await?;
        match action.value {
            0 => Ok(PlayerActions::StartDig),
            1 => Ok(PlayerActions::CancelDig),
            2 => Ok(PlayerActions::FinishDig),
            3 => Ok(PlayerActions::DropStack),
            4 => Ok(PlayerActions::DropItem),
            5 => Ok(PlayerActions::ShootArrowFinishEating),
            6 => Ok(PlayerActions::SwapHands),
            _ => Err(format!("Invalid player action: {}", action.value))
        }
    }
}

impl SizedProt for PlayerActions {
    fn prot_size(&self) -> usize {
        VarInt::from(0).prot_size()
    }
}