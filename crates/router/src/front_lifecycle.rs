use parking_lot::Mutex as ParkingMutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration as StdDuration, Instant};

#[derive(Clone)]
pub(crate) struct FrontLifecycle {
    active_connections: Arc<AtomicUsize>,
    leases: Arc<ParkingMutex<HashMap<String, FrontLease>>>,
    last_activity: Arc<ParkingMutex<Instant>>,
    idle_shutdown_after: StdDuration,
}

#[derive(Clone)]
struct FrontLease {
    kind: String,
    expires_at: Instant,
}

impl FrontLifecycle {
    pub(crate) fn new() -> Self {
        Self {
            active_connections: Arc::new(AtomicUsize::new(0)),
            leases: Arc::new(ParkingMutex::new(HashMap::new())),
            last_activity: Arc::new(ParkingMutex::new(Instant::now())),
            idle_shutdown_after: router_idle_shutdown_after(),
        }
    }

    pub(crate) fn connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
    }

    pub(crate) fn heartbeat(&self, input: &Value) -> anyhow::Result<Value> {
        let front_id = input
            .get("front_id")
            .or_else(|| input.get("frontId"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("front_id is required"))?
            .to_string();
        let kind = input
            .get("kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("gateway")
            .to_string();
        let ttl_ms = input
            .get("ttl_ms")
            .or_else(|| input.get("ttlMs"))
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .unwrap_or_else(default_front_lease_ttl_ms);
        let expires_at = Instant::now() + StdDuration::from_millis(ttl_ms);
        self.leases.lock().insert(
            front_id.clone(),
            FrontLease {
                kind: kind.clone(),
                expires_at,
            },
        );
        self.mark_activity();
        Ok(json!({
            "status": "ok",
            "front_id": front_id,
            "kind": kind,
            "ttl_ms": ttl_ms,
        }))
    }

    pub(crate) fn snapshot(&self) -> Value {
        let active_fronts = self.prune_and_count_valid_leases();
        let front_kinds = self
            .leases
            .lock()
            .values()
            .map(|lease| lease.kind.clone())
            .collect::<Vec<_>>();
        json!({
            "active_connections": self.active_connections.load(Ordering::SeqCst),
            "active_fronts": active_fronts,
            "front_kinds": front_kinds,
            "idle_shutdown_after_ms": self.idle_shutdown_after.as_millis() as u64,
            "idle_for_ms": self.last_activity.lock().elapsed().as_millis() as u64,
        })
    }

    pub(crate) fn should_shutdown_idle(
        &self,
        active_runtime_workers: usize,
        active_sessions: usize,
    ) -> bool {
        let active_fronts = self.prune_and_count_valid_leases();
        let active_connections = self.active_connections.load(Ordering::SeqCst);
        if active_fronts > 0
            || active_connections > 0
            || active_runtime_workers > 0
            || active_sessions > 0
        {
            self.mark_activity();
            return false;
        }
        self.last_activity.lock().elapsed() >= self.idle_shutdown_after
    }

    pub(crate) fn mark_activity(&self) {
        *self.last_activity.lock() = Instant::now();
    }

    fn prune_and_count_valid_leases(&self) -> usize {
        let now = Instant::now();
        let mut leases = self.leases.lock();
        leases.retain(|_, lease| lease.expires_at > now);
        leases.len()
    }
}

fn router_idle_shutdown_after() -> StdDuration {
    std::env::var("TURA_ROUTER_IDLE_SHUTDOWN_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(StdDuration::from_secs)
        .unwrap_or_else(|| StdDuration::from_secs(60))
}

fn default_front_lease_ttl_ms() -> u64 {
    std::env::var("TURA_ROUTER_FRONT_LEASE_TTL_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(15)
        .saturating_mul(1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_heartbeat_keeps_router_alive_until_lease_expires() -> anyhow::Result<()> {
        let lifecycle = FrontLifecycle {
            active_connections: Arc::new(AtomicUsize::new(0)),
            leases: Arc::new(ParkingMutex::new(HashMap::new())),
            last_activity: Arc::new(ParkingMutex::new(
                Instant::now() - StdDuration::from_millis(50),
            )),
            idle_shutdown_after: StdDuration::from_millis(10),
        };

        lifecycle.heartbeat(&json!({
            "front_id": "gateway-test",
            "kind": "gateway",
            "ttl_ms": 100,
        }))?;
        std::thread::sleep(StdDuration::from_millis(50));
        assert!(!lifecycle.should_shutdown_idle(0, 0));
        std::thread::sleep(StdDuration::from_millis(80));
        assert!(lifecycle.should_shutdown_idle(0, 0));
        Ok(())
    }

    #[test]
    fn lifecycle_active_connection_blocks_idle_shutdown() {
        let lifecycle = FrontLifecycle {
            active_connections: Arc::new(AtomicUsize::new(0)),
            leases: Arc::new(ParkingMutex::new(HashMap::new())),
            last_activity: Arc::new(ParkingMutex::new(
                Instant::now() - StdDuration::from_millis(50),
            )),
            idle_shutdown_after: StdDuration::from_millis(10),
        };

        lifecycle.connection_opened();
        std::thread::sleep(StdDuration::from_millis(20));
        assert!(!lifecycle.should_shutdown_idle(0, 0));
        lifecycle.connection_closed();
        assert!(!lifecycle.should_shutdown_idle(0, 0));
        std::thread::sleep(StdDuration::from_millis(20));
        assert!(lifecycle.should_shutdown_idle(0, 0));
    }
}
