use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::adb;

const BACKGROUND_CAPTURE_HEIGHT: u32 = 360;
const BACKGROUND_CAPTURE_QUALITY: u8 = 48;
const REQUESTED_BATCH_SIZE: usize = 6;
const BACKGROUND_BATCH_SIZE: usize = 2;
const DEVICE_SCAN_INTERVAL: Duration = Duration::from_secs(4);
const LOOP_SLEEP: Duration = Duration::from_millis(100);
const IDLE_SLEEP: Duration = Duration::from_millis(500);
const ACTIVE_CONSUMER_TTL: Duration = Duration::from_secs(8);
const HIGH_PRIORITY_FRESHNESS: Duration = Duration::from_millis(1200);
const NORMAL_PRIORITY_FRESHNESS: Duration = Duration::from_secs(4);
const BACKGROUND_FRESHNESS: Duration = Duration::from_secs(18);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PreviewPriority {
    Normal,
    High,
}

#[derive(Clone, Copy, Debug)]
struct PendingRefresh {
    priority: PreviewPriority,
    requested_at: Instant,
}

#[derive(Clone, Debug)]
pub struct CachedPreview {
    pub serial: String,
    pub jpeg: Vec<u8>,
    pub version: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone)]
pub struct PreviewHub {
    inner: Arc<PreviewHubInner>,
}

struct PreviewHubInner {
    frames: RwLock<HashMap<String, CachedPreview>>,
    requested: Mutex<HashMap<String, PendingRefresh>>,
    last_consumer_at: Mutex<Option<Instant>>,
}

impl PreviewHub {
    pub fn new() -> Self {
        let hub = Self {
            inner: Arc::new(PreviewHubInner {
                frames: RwLock::new(HashMap::new()),
                requested: Mutex::new(HashMap::new()),
                last_consumer_at: Mutex::new(None),
            }),
        };
        hub.spawn_worker();
        hub
    }

    pub fn request_refresh<I, S>(&self, serials: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.request_refresh_with_priority(serials, PreviewPriority::Normal);
    }

    pub fn request_refresh_with_priority<I, S>(&self, serials: I, priority: PreviewPriority)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.touch_consumer();
        if let Ok(mut requested) = self.inner.requested.lock() {
            for serial in serials {
                let serial = serial.into();
                if !serial.trim().is_empty() {
                    requested
                        .entry(serial)
                        .and_modify(|pending| {
                            if priority > pending.priority {
                                pending.priority = priority;
                            }
                            pending.requested_at = Instant::now();
                        })
                        .or_insert(PendingRefresh {
                            priority,
                            requested_at: Instant::now(),
                        });
                }
            }
        }
    }

    pub fn get_frame(&self, serial: &str) -> Option<CachedPreview> {
        self.inner
            .frames
            .read()
            .ok()
            .and_then(|frames| frames.get(serial).cloned())
    }

    pub fn get_or_capture(
        &self,
        serial: &str,
        target_height: u32,
        quality: u8,
    ) -> Result<CachedPreview, String> {
        if let Some(frame) = self.get_frame(serial) {
            return Ok(frame);
        }
        let frame = capture_frame(serial, target_height, quality)?;
        self.store_frame(frame.clone());
        Ok(frame)
    }

    pub fn last_updated_ms(&self) -> u64 {
        self.inner
            .frames
            .read()
            .ok()
            .and_then(|frames| frames.values().map(|frame| frame.updated_at_ms).max())
            .unwrap_or(0)
    }

    pub fn frames(&self) -> Vec<CachedPreview> {
        self.inner
            .frames
            .read()
            .map(|frames| frames.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    }

    pub fn touch_consumer(&self) {
        if let Ok(mut last_consumer_at) = self.inner.last_consumer_at.lock() {
            *last_consumer_at = Some(Instant::now());
        }
    }

    fn spawn_worker(&self) {
        let hub = self.clone();
        thread::spawn(move || hub.worker_loop());
    }

    fn worker_loop(self) {
        let mut online_serials: Vec<String> = Vec::new();
        let mut serial_cursor = 0usize;
        let mut last_scan_at = Instant::now()
            .checked_sub(DEVICE_SCAN_INTERVAL)
            .unwrap_or_else(Instant::now);

        loop {
            let has_active_consumer = self.has_active_consumer();
            let has_requested = self.has_requested();

            if !has_active_consumer && !has_requested {
                online_serials.clear();
                serial_cursor = 0;
                thread::sleep(IDLE_SLEEP);
                continue;
            }

            if has_active_consumer
                && (online_serials.is_empty() || last_scan_at.elapsed() >= DEVICE_SCAN_INTERVAL)
            {
                online_serials = adb::list_devices()
                    .map(|devices| {
                        devices
                            .into_iter()
                            .filter(|device| device.state == "device")
                            .map(|device| device.serial)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                online_serials.sort();
                if serial_cursor >= online_serials.len() {
                    serial_cursor = 0;
                }
                last_scan_at = Instant::now();
            }

            let mut batch = self.take_requested(REQUESTED_BATCH_SIZE);
            if has_active_consumer && batch.len() < REQUESTED_BATCH_SIZE && !online_serials.is_empty()
            {
                while batch.len() < REQUESTED_BATCH_SIZE + BACKGROUND_BATCH_SIZE {
                    let Some(serial) =
                        self.next_background_serial(&online_serials, &mut serial_cursor, &batch)
                    else {
                        break;
                    };
                    batch.push(serial);
                }
            }

            if batch.is_empty() {
                thread::sleep(IDLE_SLEEP);
                continue;
            }

            let (frame_tx, frame_rx) = mpsc::channel();
            for serial in batch {
                let frame_tx = frame_tx.clone();
                thread::spawn(move || {
                    let _ = frame_tx.send(capture_frame(
                        &serial,
                        BACKGROUND_CAPTURE_HEIGHT,
                        BACKGROUND_CAPTURE_QUALITY,
                    ));
                });
            }
            drop(frame_tx);
            for frame in frame_rx {
                if let Ok(frame) = frame {
                    self.store_frame(frame);
                }
            }

            thread::sleep(LOOP_SLEEP);
        }
    }

    fn take_requested(&self, limit: usize) -> Vec<String> {
        let mut requested = match self.inner.requested.lock() {
            Ok(requested) => requested,
            Err(_) => return Vec::new(),
        };
        let mut pending = requested
            .iter()
            .map(|(serial, pending)| (serial.clone(), *pending))
            .collect::<Vec<_>>();
        pending.sort_by(|left, right| {
            right
                .1
                .priority
                .cmp(&left.1.priority)
                .then_with(|| {
                    self.preview_sort_key(&left.0)
                        .cmp(&self.preview_sort_key(&right.0))
                })
                .then_with(|| left.1.requested_at.cmp(&right.1.requested_at))
        });

        let mut selected = Vec::new();
        let mut fresh = Vec::new();
        for (serial, pending) in pending {
            if self.preview_missing_or_stale(&serial, pending.priority.freshness()) {
                if selected.len() < limit {
                    selected.push(serial.clone());
                }
            } else {
                fresh.push(serial.clone());
            }
        }
        for serial in &selected {
            requested.remove(serial);
        }
        for serial in &fresh {
            requested.remove(serial);
        }
        selected
    }

    fn store_frame(&self, frame: CachedPreview) {
        if let Ok(mut frames) = self.inner.frames.write() {
            frames.insert(frame.serial.clone(), frame);
        }
    }

    fn has_requested(&self) -> bool {
        self.inner
            .requested
            .lock()
            .map(|requested| !requested.is_empty())
            .unwrap_or(false)
    }

    fn has_active_consumer(&self) -> bool {
        self.inner
            .last_consumer_at
            .lock()
            .ok()
            .and_then(|last_consumer_at| *last_consumer_at)
            .map(|instant| instant.elapsed() <= ACTIVE_CONSUMER_TTL)
            .unwrap_or(false)
    }

    fn next_background_serial(
        &self,
        online_serials: &[String],
        serial_cursor: &mut usize,
        existing_batch: &[String],
    ) -> Option<String> {
        if online_serials.is_empty() {
            return None;
        }
        if *serial_cursor >= online_serials.len() {
            *serial_cursor = 0;
        }
        for _ in 0..online_serials.len() {
            let serial = online_serials[*serial_cursor % online_serials.len()].clone();
            *serial_cursor = (*serial_cursor + 1) % online_serials.len();
            if existing_batch.iter().any(|item| item == &serial) {
                continue;
            }
            if self.is_requested(&serial) {
                continue;
            }
            if self.preview_missing_or_stale(&serial, BACKGROUND_FRESHNESS) {
                return Some(serial);
            }
        }
        None
    }

    fn is_requested(&self, serial: &str) -> bool {
        self.inner
            .requested
            .lock()
            .map(|requested| requested.contains_key(serial))
            .unwrap_or(false)
    }

    fn preview_missing_or_stale(&self, serial: &str, threshold: Duration) -> bool {
        let now_ms = timestamp_ms();
        self.inner
            .frames
            .read()
            .ok()
            .and_then(|frames| frames.get(serial).cloned())
            .map(|frame| now_ms.saturating_sub(frame.updated_at_ms) >= threshold.as_millis() as u64)
            .unwrap_or(true)
    }

    fn preview_sort_key(&self, serial: &str) -> (u8, u64) {
        let now_ms = timestamp_ms();
        self.inner
            .frames
            .read()
            .ok()
            .and_then(|frames| frames.get(serial).cloned())
            .map(|frame| {
                let age = now_ms.saturating_sub(frame.updated_at_ms);
                (1, u64::MAX.saturating_sub(age))
            })
            .unwrap_or((0, 0))
    }
}

impl PreviewPriority {
    fn freshness(self) -> Duration {
        match self {
            PreviewPriority::Normal => NORMAL_PRIORITY_FRESHNESS,
            PreviewPriority::High => HIGH_PRIORITY_FRESHNESS,
        }
    }
}

fn capture_frame(serial: &str, target_height: u32, quality: u8) -> Result<CachedPreview, String> {
    let jpeg = adb::capture_preview_jpeg(serial, target_height, quality)?;
    let now_ms = timestamp_ms();
    Ok(CachedPreview {
        serial: serial.to_string(),
        jpeg,
        version: now_ms,
        updated_at_ms: now_ms,
    })
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
