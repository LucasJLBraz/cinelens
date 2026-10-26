//! Sync: the owner data of every device merged through one shared blob, per item the most
//! recent change winning.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{Core, Error, OwnerData};

/// Where the owner's devices exchange changes (their Google Drive): one blob, read and
/// replaced whole.
pub trait SyncStorage {
    /// `None` before any device has synced.
    fn get(&mut self) -> Result<Option<Vec<u8>>, String>;
    fn put(&mut self, bytes: &[u8]) -> Result<(), String>;
}

impl Core {
    /// Merges what the other devices synced into the owner data, then shares the result. Never
    /// asks the owner anything: every conflict is settled by which change is most recent.
    // ponytail: read-merge-write with no lock; two devices syncing in the same instant can drop
    // one's change until it changes again. Use Drive's revision check if that ever bites.
    pub fn sync(&mut self, storage: &mut dyn SyncStorage) -> Result<(), Error> {
        let remote = storage.get().map_err(Error::Sync)?;
        if let Some(bytes) = &remote {
            let theirs: OwnerData =
                serde_json::from_slice(bytes).map_err(|e| Error::InvalidOwnerData(e.to_string()))?;
            self.owner.merge(theirs);
            self.recompute();
        }
        let merged = self.owner_data();
        if remote.as_ref() != Some(&merged) {
            storage.put(&merged).map_err(Error::Sync)?;
        }
        Ok(())
    }
}

impl OwnerData {
    fn merge(&mut self, theirs: OwnerData) {
        for (id, history) in theirs.notas {
            for entry in history {
                self.add_nota(id, entry);
            }
        }
        self.nao_vi.extend(theirs.nao_vi);
        // The earliest announcement, so every device settles on the same.
        for (id, at) in theirs.notificados {
            self.notificados.entry(id).and_modify(|t| *t = (*t).min(at)).or_insert(at);
        }
        let (mine, other) = (&self.changed, &theirs.changed);
        lww_set("quero_assistir", &mut self.quero_assistir, &theirs.quero_assistir, mine, other);
        lww_set("ja_vi", &mut self.ja_vi, &theirs.ja_vi, mine, other);
        lww_set("nao_me_interessa", &mut self.nao_me_interessa, &theirs.nao_me_interessa, mine, other);
        lww_set("fas", &mut self.fas, &theirs.fas, mine, other);
        lww("agora_nao", &mut self.agora_nao, &theirs.agora_nao, mine, other);
        lww("pesou", &mut self.pesou, &theirs.pesou, mine, other);
        let (s, t) = (&mut self.settings, &theirs.settings);
        lww_value("settings/decay", &mut s.decay, &t.decay, mine, other);
        lww_value("settings/display_mode", &mut s.display_mode, &t.display_mode, mine, other);
        lww_value("settings/servicos", &mut s.servicos, &t.servicos, mine, other);
        lww_value("settings/pesou_oculto", &mut s.pesou_oculto, &t.pesou_oculto, mine, other);
        lww_value("settings/sem_notificacao", &mut s.sem_notificacao, &t.sem_notificacao, mine, other);
        for (key, at) in theirs.changed {
            let stamp = self.changed.entry(key).or_insert(at);
            *stamp = (*stamp).max(at);
        }
    }
}

/// The key `OwnerData::changed` stamps an item under.
pub(crate) fn key(field: &str, id: u32) -> String {
    format!("{field}/{id}")
}

/// Per item of `field` either side has: the side that changed it last keeps its state, present
/// or removed. Items neither side stamped (imported, or given before sync) are kept from both.
fn lww<V: Clone + Serialize>(
    field: &str,
    mine: &mut BTreeMap<u32, V>,
    theirs: &BTreeMap<u32, V>,
    my_stamps: &BTreeMap<String, i64>,
    their_stamps: &BTreeMap<String, i64>,
) {
    let ids: Vec<u32> = mine.keys().chain(theirs.keys()).copied().collect();
    for id in ids {
        let key = key(field, id);
        let (a, b) = (my_stamps.get(&key), their_stamps.get(&key));
        let take = match (mine.get(&id), theirs.get(&id)) {
            _ if b > a => true,
            _ if a > b => false,
            (None, Some(_)) => true,
            (Some(m), Some(t)) => tie_break(m, t),
            _ => false,
        };
        if take {
            match theirs.get(&id) {
                Some(v) => _ = mine.insert(id, v.clone()),
                None => _ = mine.remove(&id),
            }
        }
    }
}

fn lww_set(
    field: &str,
    mine: &mut std::collections::BTreeSet<u32>,
    theirs: &std::collections::BTreeSet<u32>,
    my_stamps: &BTreeMap<String, i64>,
    their_stamps: &BTreeMap<String, i64>,
) {
    let mut map: BTreeMap<u32, ()> = mine.iter().map(|&id| (id, ())).collect();
    let other: BTreeMap<u32, ()> = theirs.iter().map(|&id| (id, ())).collect();
    lww(field, &mut map, &other, my_stamps, their_stamps);
    *mine = map.into_keys().collect();
}

/// A setting: the side that changed it last wins. Set before sync existed on both sides: a
/// value the owner chose wins over the default of a device that never had one.
fn lww_value<V: Clone + Default + PartialEq + Serialize>(
    key: &str,
    mine: &mut V,
    theirs: &V,
    my_stamps: &BTreeMap<String, i64>,
    their_stamps: &BTreeMap<String, i64>,
) {
    let (a, b) = (my_stamps.get(key), their_stamps.get(key));
    let default = V::default();
    let take = match () {
        _ if a != b => b > a,
        _ if a.is_none() && (*mine == default) != (*theirs == default) => *mine == default,
        _ => tie_break(mine, theirs),
    };
    if take {
        *mine = theirs.clone();
    }
}

/// Changed in the same second on both sides: either value, as long as both sides pick the same.
fn tie_break<V: Serialize>(mine: &V, theirs: &V) -> bool {
    let json = |v: &V| serde_json::to_string(v).expect("owner data serializes");
    json(theirs) > json(mine)
}
