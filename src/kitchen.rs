use crate::client::{Action, Order};
use crate::client::{COLD, COOLER, DISCARD, HEATER, HOT, MOVE, PICKUP, PLACE, ROOM, SHELF};

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};

const COOLER_CAPACITY: usize = 6;
const HEATER_CAPACITY: usize = 6;
const SHELF_CAPACITY: usize = 12; // TODO: maybe make this configurable?

const DEGRADATION_RATE_IDEAL: i64 = 1;
const DEGRADATION_RATE_NON_IDEAL: i64 = 2;

#[derive(Debug, Clone)]
struct StoredOrder {
    order: Order,
    placed_at: SystemTime,
    current_temp: String,
}

impl StoredOrder {
    fn get_storage_temp(storage_location: &str) -> &str {
        match storage_location {
            HEATER => HOT,
            COOLER => COLD,
            SHELF => ROOM,
            _ => ROOM,
        }
    }

    // calc remaining freshness
    fn remaining_freshness(&self, now: SystemTime) -> i64 {
        let elapsed = now
            .duration_since(self.placed_at)
            .unwrap_or_default()
            .as_secs() as i64;

        // could optimize this later but works for now

        let storage_temp = Self::get_storage_temp(&self.current_temp);
        let degradation_rate = if self.order.temp == storage_temp {
            DEGRADATION_RATE_IDEAL
        } else {
            DEGRADATION_RATE_NON_IDEAL
        };

        let degraded_freshness = elapsed * degradation_rate;
        self.order.freshness as i64 - degraded_freshness
    }

    fn is_expired(&self, now: SystemTime) -> bool {
        self.remaining_freshness(now) <= 0
    }
}

// priority queue entry
#[derive(Debug, Clone, Eq, PartialEq)]
struct OrderEntry {
    order_id: String,
    expires_at: i64, // Unix timestamp in microseconds
}

impl Ord for OrderEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.expires_at.cmp(&other.expires_at)
    }
}

impl PartialOrd for OrderEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct Kitchen {
    cooler: Arc<Mutex<VecDeque<StoredOrder>>>,
    heater: Arc<Mutex<VecDeque<StoredOrder>>>,
    shelf: Arc<Mutex<HashMap<String, StoredOrder>>>,
    shelf_queue: Arc<Mutex<BinaryHeap<Reverse<OrderEntry>>>>,

    actions: Arc<Mutex<Vec<Action>>>,

    // make sure timestamps are monotonic
    last_timestamp: AtomicU64,
}

impl Kitchen {
    pub fn new() -> Self {
        Self {
            cooler: Arc::new(Mutex::new(VecDeque::new())),
            heater: Arc::new(Mutex::new(VecDeque::new())),
            shelf: Arc::new(Mutex::new(HashMap::new())),
            shelf_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            actions: Arc::new(Mutex::new(Vec::new())),
            last_timestamp: AtomicU64::new(0),
        }
    }

    fn record_action(
        &self,
        order_id: String,
        action_type: &str,
        target: &str,
        timestamp: SystemTime,
    ) {
        let provided_timestamp_micros =
            timestamp.duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;

        // need to ensure monotonicity across threads
        let monotonic_timestamp_micros = loop {
            let last = self.last_timestamp.load(AtomicOrdering::Acquire);
            let candidate = provided_timestamp_micros.max(last + 1);

            match self.last_timestamp.compare_exchange_weak(
                last,
                candidate,
                AtomicOrdering::Release,
                AtomicOrdering::Acquire,
            ) {
                Ok(_) => break candidate,
                Err(_actual) => {
                    // retry
                    continue;
                }
            }
        };

        let monotonic_timestamp =
            UNIX_EPOCH + std::time::Duration::from_micros(monotonic_timestamp_micros);

        let action = Action::new(&order_id, action_type, target, monotonic_timestamp);
        if let Ok(mut actions) = self.actions.lock() {
            actions.push(action.clone());
        }
        println!(
            "[{}] {}: {} -> {}",
            monotonic_timestamp_micros, action_type, order_id, target
        );
    }

    pub fn place_order(&self, order: Order, timestamp: SystemTime) {
        let stored = StoredOrder {
            order: order.clone(),
            placed_at: timestamp,
            current_temp: String::new(),
        };

        let ideal_target = match order.temp.as_str() {
            HOT => HEATER,
            COLD => COOLER,
            _ => SHELF,
        };

        let placed = if order.temp == HOT || order.temp == COLD {
            if self.try_place_in_storage(&stored, ideal_target, timestamp) {
                true
            } else {
                self.try_place_on_shelf(&stored, timestamp)
            }
        } else {
            self.try_place_on_shelf(&stored, timestamp)
        };

        if !placed {
            if order.temp == HOT || order.temp == COLD {
                if self.try_move_to_shelf_from_storage(ideal_target, timestamp) {
                    self.force_place_in_storage(&stored, ideal_target, timestamp);
                } else {
                    if self.try_place_on_shelf(&stored, timestamp) {
                        return;
                    }
                    self.discard_from_shelf(timestamp);
                    self.force_place_on_shelf(&stored, timestamp);
                }
            } else {
                self.discard_from_shelf(timestamp);
                self.force_place_on_shelf(&stored, timestamp);
            }
        }
    }

    fn try_place_in_storage(
        &self,
        stored: &StoredOrder,
        target: &str,
        timestamp: SystemTime,
    ) -> bool {
        let mut storage = if target == COOLER {
            self.cooler.lock().unwrap()
        } else {
            self.heater.lock().unwrap()
        };

        let capacity = if target == COOLER {
            COOLER_CAPACITY
        } else {
            HEATER_CAPACITY
        };
        if storage.len() >= capacity {
            return false;
        }

        let mut stored = stored.clone();
        stored.current_temp = target.to_string();
        let order_id = stored.order.id.clone();
        storage.push_back(stored);
        self.record_action(order_id, PLACE, target, timestamp);
        true
    }

    fn try_place_on_shelf(&self, stored: &StoredOrder, timestamp: SystemTime) -> bool {
        let mut shelf = self.shelf.lock().unwrap();
        if shelf.len() >= SHELF_CAPACITY {
            return false;
        }

        let mut stored = stored.clone();
        stored.current_temp = SHELF.to_string();

        let order_id = stored.order.id.clone();
        let expires_at = self.calculate_expiration(&stored, stored.placed_at);
        let entry = OrderEntry {
            order_id: order_id.clone(),
            expires_at,
        };

        shelf.insert(order_id.clone(), stored);

        self.shelf_queue.lock().unwrap().push(Reverse(entry));
        drop(shelf);
        self.record_action(order_id, PLACE, SHELF, timestamp);
        true
    }

    fn force_place_on_shelf(&self, stored: &StoredOrder, timestamp: SystemTime) {
        let mut shelf = self.shelf.lock().unwrap();

        if shelf.len() >= SHELF_CAPACITY {
            panic!("force_place_on_shelf called when shelf is full");
        }

        let mut stored = stored.clone();
        stored.current_temp = SHELF.to_string();

        let order_id = stored.order.id.clone();
        let expires_at = self.calculate_expiration(&stored, stored.placed_at);
        let entry = OrderEntry {
            order_id: order_id.clone(),
            expires_at,
        };

        shelf.insert(order_id.clone(), stored);
        self.shelf_queue.lock().unwrap().push(Reverse(entry));
        drop(shelf);
        self.record_action(order_id, PLACE, SHELF, timestamp);
    }

    fn force_place_in_storage(&self, stored: &StoredOrder, target: &str, timestamp: SystemTime) {
        let mut storage = if target == COOLER {
            self.cooler.lock().unwrap()
        } else {
            self.heater.lock().unwrap()
        };

        let capacity = if target == COOLER {
            COOLER_CAPACITY
        } else {
            HEATER_CAPACITY
        };
        if storage.len() >= capacity {
            panic!("force_place_in_storage called when storage is full");
        }

        let mut stored = stored.clone();
        stored.current_temp = target.to_string();
        let order_id = stored.order.id.clone();
        storage.push_back(stored);
        self.record_action(order_id, PLACE, target, timestamp);
    }

    fn try_move_to_shelf_from_storage(&self, source: &str, timestamp: SystemTime) -> bool {
        let shelf = self.shelf.lock().unwrap();
        if shelf.len() >= SHELF_CAPACITY {
            drop(shelf);
            self.discard_from_shelf(timestamp);
        } else {
            drop(shelf);
        }

        let mut storage = if source == COOLER {
            self.cooler.lock().unwrap()
        } else {
            self.heater.lock().unwrap()
        };

        if storage.is_empty() {
            drop(storage);
            return false;
        }

        let stored = storage.pop_front().unwrap();
        let order_id = stored.order.id.clone();
        drop(storage);

        let mut moved = stored.clone();
        moved.current_temp = SHELF.to_string();

        let mut shelf = self.shelf.lock().unwrap();
        let expires_at = self.calculate_expiration(&moved, moved.placed_at);
        let entry = OrderEntry {
            order_id: order_id.clone(),
            expires_at,
        };

        shelf.insert(order_id.clone(), moved);
        self.shelf_queue.lock().unwrap().push(Reverse(entry));
        drop(shelf);
        self.record_action(order_id, MOVE, SHELF, timestamp);
        true
    }

    fn discard_from_shelf(&self, timestamp: SystemTime) {
        let mut shelf = self.shelf.lock().unwrap();
        let mut queue = self.shelf_queue.lock().unwrap();

        while let Some(Reverse(entry)) = queue.pop() {
            if let Some(_stored) = shelf.remove(&entry.order_id) {
                self.record_action(entry.order_id, DISCARD, SHELF, timestamp);
                return;
            }
        }

        if shelf.is_empty() {
            panic!("discard_from_shelf called but shelf is empty");
        }
        panic!("discard_from_shelf failed");
    }

    fn calculate_expiration(&self, stored: &StoredOrder, _now: SystemTime) -> i64 {
        let storage_temp = StoredOrder::get_storage_temp(&stored.current_temp);
        let degradation_rate = if stored.order.temp == storage_temp {
            DEGRADATION_RATE_IDEAL
        } else {
            DEGRADATION_RATE_NON_IDEAL
        };

        let seconds_until_expiration = stored.order.freshness as f64 / degradation_rate as f64;
        let microseconds_until_expiration = (seconds_until_expiration * 1_000_000.0) as u64;

        stored
            .placed_at
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as i64
            + microseconds_until_expiration as i64
    }

    pub fn pickup_order(&self, order_id: &str, timestamp: SystemTime) {
        // check cooler first
        {
            let mut cooler = self.cooler.lock().unwrap();
            if let Some(pos) = cooler.iter().position(|o| o.order.id == order_id) {
                let stored = cooler.remove(pos).unwrap();
                if stored.is_expired(timestamp) {
                    self.record_action(order_id.to_string(), DISCARD, COOLER, timestamp);
                } else {
                    self.record_action(order_id.to_string(), PICKUP, COOLER, timestamp);
                }
                return;
            }
        }

        {
            let mut heater = self.heater.lock().unwrap();
            if let Some(pos) = heater.iter().position(|o| o.order.id == order_id) {
                let stored = heater.remove(pos).unwrap();
                if stored.is_expired(timestamp) {
                    self.record_action(order_id.to_string(), DISCARD, HEATER, timestamp);
                } else {
                    self.record_action(order_id.to_string(), PICKUP, HEATER, timestamp);
                }
                return;
            }
        }

        // then shelf
        {
            let mut shelf = self.shelf.lock().unwrap();
            if let Some(stored) = shelf.remove(&order_id.to_string()) {
                let mut queue = self.shelf_queue.lock().unwrap();
                queue.retain(|Reverse(entry)| entry.order_id != order_id);
                drop(queue);

                if stored.is_expired(timestamp) {
                    self.record_action(order_id.to_string(), DISCARD, SHELF, timestamp);
                } else {
                    self.record_action(order_id.to_string(), PICKUP, SHELF, timestamp);
                }
            }
        }
    }

    pub fn get_actions(&self) -> Vec<Action> {
        let mut actions = self.actions.lock().unwrap().clone();
        actions.sort_by_key(|a| a.timestamp);
        actions
    }
}
