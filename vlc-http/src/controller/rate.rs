// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

pub struct Limiter {
    interval: shared::TimeDifference,
    last_time: Option<shared::Time>,
}
impl Limiter {
    pub fn new(interval_millis: u32) -> Self {
        Self {
            interval: shared::TimeDifference::milliseconds(interval_millis.into()),
            last_time: None,
        }
    }
    pub async fn enter(&mut self) {
        let now = shared::time_now();
        if let Some(last_act_time) = self.last_time {
            let since_last_act = now - last_act_time;
            let remaining_delay = self.interval - since_last_act;
            if let Ok(delay) = remaining_delay.to_std() {
                println!("waiting {delay:?}");
                tokio::time::sleep(delay).await;
            }
        }
        self.last_time = Some(now);
    }
}
