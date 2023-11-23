use async_nbt::{NbtCompound, NbtList, NbtTag};
use serde_json::Value;
use tokio::fs;

fn parse_registry_value(val: &Value) -> Result<NbtTag, String> {
    Ok(match val {
        Value::Null => return Err("Null value in registry".into()),
        Value::Bool(_) => return Err("Bool value in registry".into()),
        Value::Number(v) => {
            if v.is_f64() {
                NbtTag::Float(v.as_f64().unwrap() as f32)
            } else if v.is_i64() {
                let v = v.as_i64().unwrap();
                if v > i32::MAX as i64 || v < i32::MIN as i64 {
                    NbtTag::Long(v)
                } else {
                    NbtTag::Int(v as i32)
                }
            } else {
                return Err("Can't decide on number type".into());
            }
        }
        Value::String(v) => NbtTag::String(v.clone()),
        Value::Array(v) => NbtTag::List(parse_registry_array(v)?),
        Value::Object(v) => NbtTag::Compound(parse_registry_object(v)?),
    })
}

fn parse_registry_array(array: &Vec<Value>) -> Result<NbtList, String> {
    let mut list = NbtList::new();
    for value in array {
        let tag: NbtTag = parse_registry_value(value)?;
        list.push(tag);
    }
    Ok(list)
}

fn parse_registry_object(object: &serde_json::Map<String, Value>) -> Result<NbtCompound, String> {
    let mut compound = NbtCompound::new();
    for (key, value) in object {
        let tag: NbtTag = parse_registry_value(value)?;
        compound.insert(key.clone(), tag);
    }
    Ok(compound)
}

pub(crate) async fn load_registry() -> Result<NbtCompound, String> {
    let registry_json = fs::read_to_string("assets/registry.json").await.unwrap();
    let registry_json = serde_json::from_str::<Value>(&registry_json).unwrap();
    let root = registry_json.as_object().unwrap();

    // for key in registry_json object, insert a new NbtCompound with the key as name
    // and the value as value
    parse_registry_object(root)
}
