use std::time::{Duration, Instant, SystemTime};

use dashmap::DashMap;
use futures::TryFutureExt;
use tokio::sync::{mpsc, oneshot};
use ulid::Ulid;
type WaitListReceiver = mpsc::Receiver<oneshot::Sender<Vec<u8>>>;
type WaitListSender = mpsc::Sender<oneshot::Sender<Vec<u8>>>;

pub(crate) struct LegacyCache {
    cache: DashMap<String, CachedValue>,
    wait_list: DashMap<String, (Ulid, WaitListSender, WaitListReceiver)>,
}

struct CachedValue {
    data: Vec<u8>,
    expires_at: Option<Instant>,
}

impl LegacyCache {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
            wait_list: DashMap::new(),
        }
    }

    /// Gets a value from the cache by key. If this function returns None, the caller must set a new one.
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        if let dashmap::Entry::Occupied(entry) = self.cache.entry(key.to_string()) {
            if entry.get().expires_at.map(|expiry| expiry < Instant::now()) == Some(true) {
                entry.remove();
            } else {
                return Some(entry.get().data.clone());
            }
        }

        let Some((wait_list_sender, wait_list_id)) = self.get_or_create_wait_list(key).await else {
            // This will short-circuit a `None` out if there is no wait list. The function creates a new list,
            // and the caller must call set with a new value. Subsequent calls will get the wait list
            // and yield until the first caller creates the value.
            return None;
        };

        // We have to have a timeout here, because the guest can do IO to get the cache value in the
        // init. If this never finishes, we will leak memory when new callers are added to the list.
        let fut = tokio::time::timeout(Duration::from_secs(5), async move {
            let (value_sender, value_receiver) = oneshot::channel();
            wait_list_sender.send(value_sender).await.ok()?;
            value_receiver.await.ok()
        });

        let fut = fut.inspect_err(|_| {
            tracing::error!("timed out waiting for cached value in extension cache to be available");
        });

        if let Some(value) = fut.await.ok().flatten() {
            return Some(value);
        };

        // This happens only if our wait list timed out. We must clean the list so we do not leak
        // memory.
        if self
            .wait_list
            .remove_if(key, |_, (id, _, _)| *id == wait_list_id)
            .is_some()
        {
            let now = SystemTime::now();

            let timestamp = Duration::from_millis(wait_list_id.timestamp_ms());
            let created_at = SystemTime::UNIX_EPOCH.checked_add(timestamp);

            let time_ago = created_at
                .and_then(|created_at| now.duration_since(created_at).ok())
                .unwrap_or_default()
                .as_secs();

            tracing::info!("Removed dead wait extension list, created {time_ago}s ago");
        }

        None
    }

    /// Sets a value in the cache with an optional time-to-live duration in milliseconds.
    pub async fn set(&self, key: &str, value: Vec<u8>, ttl_ms: Option<u64>) {
        let cached_value = CachedValue {
            data: value.clone(),
            expires_at: ttl_ms.map(|ms| Instant::now() + std::time::Duration::from_millis(ms)),
        };

        self.cache.insert(key.to_string(), cached_value);

        // We remove the wait list so subsequent calls do not add themselves to the list. The value
        // is already in the cache. We use receive all listeners from the wait list, and send the
        // new value for them so they can continue execution.
        if let Some((_, (_, _, mut receiver))) = self.wait_list.remove(key) {
            while let Ok(waiter) = receiver.try_recv() {
                let _ = waiter.send(value.clone()).ok();
            }
        }
    }

    /// Gets or creates a wait list for the given cache key. The first caller to a cache value that is
    /// missing will create a new wait list, this function returns None and the caller must initialize
    /// a new value in the guest, and set a new value in the cache.
    ///
    /// The subsequent callers for this value will get the wait list, and add themselves to it.
    /// When the first caller sets a new value, this will send the value to everybody waiting
    /// in the wait list.
    async fn get_or_create_wait_list(&self, key: &str) -> Option<(WaitListSender, Ulid)> {
        let mut created = false;

        let entry = self
            .wait_list
            .entry(key.to_string())
            .and_modify(|(id, sender, receiver)| {
                if sender.is_closed() {
                    let (new_sender, new_receiver) = mpsc::channel::<oneshot::Sender<Vec<u8>>>(1024);

                    *id = Ulid::new();
                    *sender = new_sender;
                    *receiver = new_receiver;
                    created = true;
                }
            })
            .or_insert_with(|| {
                created = true;

                let (sender, receiver) = mpsc::channel(1024);
                (Ulid::new(), sender, receiver)
            });

        if created {
            None
        } else {
            Some((entry.1.clone(), entry.0))
        }
    }
}
