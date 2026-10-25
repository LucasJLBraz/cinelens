//! Catálogo deltas: what changed from one day's catálogo to the next, so the app downloads
//! only that. A delta is the new catálogo's fields except `movies`, plus `from` (the data
//! version it applies to), `movies` (those new or changed) and `removed` (their tmdb ids).

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::Error;

fn parse(bytes: &[u8]) -> Result<Map<String, Value>, Error> {
    match serde_json::from_slice(bytes) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(Error::InvalidArtifact("not an object".into())),
        Err(e) => Err(Error::InvalidArtifact(e.to_string())),
    }
}

fn movies(map: &mut Map<String, Value>) -> Vec<Value> {
    match map.remove("movies") {
        Some(Value::Array(movies)) => movies,
        _ => Vec::new(),
    }
}

fn id(movie: &Value) -> Option<u64> {
    movie["tmdb_id"].as_u64()
}

/// The delta that turns catálogo `old` into `new`.
pub fn catalog_delta(old: &[u8], new: &[u8]) -> Result<Vec<u8>, Error> {
    let mut old = parse(old)?;
    let mut delta = parse(new)?;
    let before: HashMap<u64, Value> = movies(&mut old).into_iter().filter_map(|m| Some((id(&m)?, m))).collect();
    let after = movies(&mut delta);
    let kept: HashSet<u64> = after.iter().filter_map(id).collect();
    let mut removed: Vec<u64> = before.keys().filter(|id| !kept.contains(id)).copied().collect();
    removed.sort_unstable();
    let changed: Vec<Value> = after.into_iter().filter(|m| id(m).and_then(|id| before.get(&id)) != Some(m)).collect();
    delta.insert("from".into(), old.remove("data_version").unwrap_or(Value::Null));
    delta.insert("movies".into(), changed.into());
    delta.insert("removed".into(), removed.into());
    Ok(serde_json::to_vec(&delta).expect("catálogo serializes"))
}

/// Catálogo `catalog` with `delta` applied: changed movies in place, new ones at the end.
/// Refused unless the delta is from this catálogo's data version.
pub fn apply_catalog_delta(catalog: &[u8], delta: &[u8]) -> Result<Vec<u8>, Error> {
    let mut catalog = parse(catalog)?;
    let mut delta = parse(delta)?;
    let from = delta.remove("from");
    if from.as_ref() != catalog.get("data_version") {
        return Err(Error::InvalidArtifact(format!("delta from {from:?}, catálogo is {:?}", catalog.get("data_version"))));
    }
    let removed: HashSet<u64> = match delta.remove("removed") {
        Some(Value::Array(ids)) => ids.iter().filter_map(Value::as_u64).collect(),
        _ => HashSet::new(),
    };
    let mut changed: HashMap<u64, Value> = movies(&mut delta).into_iter().filter_map(|m| Some((id(&m)?, m))).collect();
    let mut updated: Vec<Value> = movies(&mut catalog)
        .into_iter()
        .filter(|m| id(m).is_some_and(|id| !removed.contains(&id)))
        .map(|m| id(&m).and_then(|id| changed.remove(&id)).unwrap_or(m))
        .collect();
    let mut new: Vec<Value> = changed.into_values().collect();
    new.sort_by_key(id);
    updated.extend(new);
    // Every other field (data version, genres, providers, trending) is the new catálogo's.
    catalog.extend(delta);
    catalog.insert("movies".into(), updated.into());
    Ok(serde_json::to_vec(&catalog).expect("catálogo serializes"))
}
