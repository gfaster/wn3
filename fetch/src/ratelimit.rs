use std::{collections::BTreeMap, ops::DerefMut, sync::{Arc, Mutex as StdMutex}, time::{Duration, Instant}};

// use tokio::{sync::Mutex, time::{self, Interval}};

type Rls = StdMutex<BTreeMap<Box<str>, RateLimit>>;

static LIMITS: Rls = Rls::new(BTreeMap::new());

#[derive(Clone)]
struct RateLimit { 
    interval: Arc<StdMutex<(Instant, Duration)>>,
}


impl RateLimit {
    pub fn new(period: Duration) -> Self {
        Self {
            interval: Arc::new(StdMutex::new((Instant::now() - period, period))),
        }
    }

    pub fn acquire(&self) {
        let mut lock = self.interval.lock().unwrap();
        let (last, ref dur) = lock.deref_mut();
        let now = Instant::now();
        let elapsed = now.duration_since(*last);
        if elapsed < *dur {
            std::thread::sleep(*dur - elapsed);
            *last = Instant::now();
        } else {
            *last = now;
        }
    }
}

fn get_limiter(s: &str, default_period: Duration) -> RateLimit {
    let mut lock = LIMITS.lock().unwrap();
    if let Some(l) = lock.get(s).clone() {
        l.clone()
    } else {
        let l = RateLimit::new(default_period);
        lock.insert(s.into(), l.clone());
        l
    }
}

pub fn wait_your_turn(s: &str, default_period: Duration) {
    get_limiter(s, default_period).acquire()
}

#[cfg(test)]
mod tests {
    use super::*;

}
