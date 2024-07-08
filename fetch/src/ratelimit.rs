use std::{collections::BTreeMap, sync::{Arc, Mutex as StdMutex}, time::Duration};

use tokio::{sync::Mutex, time::{self, Interval}};

type Rls = StdMutex<BTreeMap<Box<str>, RateLimit>>;

static LIMITS: Rls = Rls::new(BTreeMap::new());

// #[derive(Clone)]
// struct RateLimit { 
//     interval: Arc<StdMutex<Interval>>,
// }
//
//
// impl RateLimit {
//     pub fn new(period: Duration) -> Self {
//         let mut interval = time::interval(period);
//         interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
//         Self {
//             interval: Arc::new(StdMutex::new(interval)),
//         }
//     }
//
//     pub fn poll_acquire(&self, cx: &mut Context) -> Poll<Instant> {
//         self.interval.lock().unwrap().poll_tick(cx)
//     }
//
//     pub async fn acquire(&self) {
//         poll_fn(|cx| self.poll_acquire(cx)).await;
//     }
// }

#[derive(Clone)]
struct RateLimit { 
    interval: Arc<Mutex<Interval>>,
}


impl RateLimit {
    pub fn new(period: Duration) -> Self {
        let mut interval = time::interval(period);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        Self {
            interval: Arc::new(Mutex::new(interval)),
        }
    }

    pub async fn acquire(&self) {
        self.interval.lock().await.tick().await;
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

pub async fn wait_your_turn(s: &str, default_period: Duration) {
    get_limiter(s, default_period).acquire().await
}

#[cfg(test)]
mod tests {
    use tokio::{join, task::JoinSet, time::Instant};

    use super::*;

    #[tokio::test]
    async fn seq() {
        tokio::time::pause();
        let start = Instant::now();
        let lmt = RateLimit::new(Duration::from_millis(300));
        lmt.acquire().await;
        lmt.acquire().await;
        lmt.acquire().await;
        let elapsed = Instant::now().duration_since(start).as_millis();
        assert!(elapsed >= 600 && elapsed < 602, "elapsed: {elapsed}");
    }

    #[tokio::test]
    async fn race() {
        tokio::time::pause();
        let start = Instant::now();
        let lmt = RateLimit::new(Duration::from_millis(300));
        join!(lmt.acquire(), lmt.acquire(), lmt.acquire());
        let elapsed = Instant::now().duration_since(start).as_millis();
        assert!(elapsed >= 600 && elapsed < 602, "elapsed: {elapsed}");
    }

    #[tokio::test]
    async fn race_more() {
        tokio::time::pause();
        let iters = 100;
        let millis = 30;
        let start = Instant::now();
        let lmt = RateLimit::new(Duration::from_millis(millis as u64));
        let mut set = JoinSet::new();
        for _ in 0..iters {
            let lmt = lmt.clone();
            set.spawn(async move {
                lmt.acquire().await
            });
        }

        while set.join_next().await.is_some() {
        }

        let elapsed = Instant::now().duration_since(start).as_millis();
        let lower = millis * (iters - 1);
        let margin = (millis * 3 / 4).min(5);
        assert!(elapsed >= lower && elapsed < lower + margin, "elapsed: {elapsed}, lower: {lower}");
    }

}
