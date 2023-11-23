use async_nbt::io::Flavor;
use async_nbt::NbtCompound;
use async_trait::async_trait;
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

#[derive(Debug)]
pub(crate) struct Position {
    x: i32,
    // actual size: 26 bits
    z: i32,
    // actual size: 26 bits
    y: i32, // actual size: 12 bits
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

#[derive(Debug)]
pub(crate) struct Recipe {
    typ: String,
    id: String,
    data: RecipeKind,
}

#[derive(Debug)]
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

#[derive(SizedProt, WriteProt, Debug)]
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

#[derive(SizedProt, WriteProt, ReadProt, Debug)]
pub(crate) struct TagGroup {
    typ: String, // minecraft:block, minecraft:item, minecraft:fluid, minecraft:entity_type, and minecraft:game_event
    tags: SizedVec<Tag>,
}

#[derive(SizedProt, WriteProt, ReadProt, Debug)]
pub(crate) struct Tag {
    name: String,
    types: SizedVec<VarInt>, // numeric IDs of the given type (block, item, etc.)
}
