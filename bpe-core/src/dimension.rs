//! Keyed dimension tables (e.g. blacklists, quotas, configuration) used by the
//! SQL functions `_dim_has(dim_id, key)` and `_dim_get(dim_id, key)`.
//!
//! Tables are identified by a small `u16` id returned from `def_dimension`.
//! Lookups are lock-protected (RwLock) so they can be called from the JIT filter
//! path (main thread) while the application updates tables from callbacks.

use globalvar::{def_global_ptr, get_global, get_global_mut};
use hashbrown::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex, Once, RwLock,
};

/// Bumped on every dimension update/remove so mappers can detect stale hit
/// bitmaps (a filter using `_dim_has/_dim_get` snapshot the dimension state
/// at insert time) and rebuild them on the engine thread.
static DIM_VERSION: AtomicU64 = AtomicU64::new(0);

/// Current dimension version (0 = no updates ever).
pub(crate) fn dim_version() -> u64 {
    DIM_VERSION.load(Ordering::Relaxed)
}

fn bump_version() {
    DIM_VERSION.fetch_add(1, Ordering::Relaxed);
}

pub(crate) struct Dimension {
    map: RwLock<HashMap<i64, i64>>,
}

static mut PTR_DIMS: u64 = 0;
static DIM_LOCK: Mutex<()> = Mutex::new(());

fn init_dims() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        PTR_DIMS = def_global_ptr(Vec::<&'static mut Dimension>::new());
    });
}

/// Allocates a new dimension table and returns its id.
pub fn def_dimension() -> Option<u16> {
    init_dims();
    let _guard = DIM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dims = get_global_mut::<Vec<&'static mut Dimension>>(unsafe { PTR_DIMS });
    let id = dims.len();
    if id > u16::MAX as usize {
        log::warn!("too many dimensions");
        return None;
    }
    dims.push(Box::leak(Box::new(Dimension {
        map: RwLock::new(HashMap::new()),
    })));
    Some(id as u16)
}

/// Inserts/updates a key -> value entry.
pub fn update_dimension(id: u16, key: i64, value: i64) {
    init_dims();
    let _guard = DIM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dims = get_global_mut::<Vec<&'static mut Dimension>>(unsafe { PTR_DIMS });
    if let Some(dim) = dims.get_mut(id as usize) {
        if let Ok(mut map) = dim.map.write() {
            map.insert(key, value);
            bump_version();
        }
    } else {
        log::warn!("dimension [{id}] is not defined");
    }
}

/// Removes a key from the dimension table.
pub fn remove_dimension(id: u16, key: i64) {
    init_dims();
    let _guard = DIM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dims = get_global_mut::<Vec<&'static mut Dimension>>(unsafe { PTR_DIMS });
    if let Some(dim) = dims.get_mut(id as usize) {
        if let Ok(mut map) = dim.map.write() {
            map.remove(&key);
            bump_version();
        }
    } else {
        log::warn!("dimension [{id}] is not defined");
    }
}

/// Returns true if the key exists in the dimension table.
pub(crate) fn dim_contains(id: u16, key: i64) -> bool {
    init_dims();
    let _guard = DIM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dims = get_global::<Vec<&'static mut Dimension>>(unsafe { PTR_DIMS });
    match dims.get(id as usize) {
        Some(dim) => dim.map.read().map(|m| m.contains_key(&key)).unwrap_or(false),
        None => {
            log::warn!("dimension [{id}] is not defined");
            false
        }
    }
}

/// Returns the value for the key, or 0 if absent.
pub(crate) fn dim_get(id: u16, key: i64) -> i64 {
    init_dims();
    let _guard = DIM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dims = get_global::<Vec<&'static mut Dimension>>(unsafe { PTR_DIMS });
    match dims.get(id as usize) {
        Some(dim) => dim.map.read().map(|m| m.get(&key).copied().unwrap_or(0)).unwrap_or(0),
        None => {
            log::warn!("dimension [{id}] is not defined");
            0
        }
    }
}
