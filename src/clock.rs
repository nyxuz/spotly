use std::time::{Duration, Instant};

use mpris::PlaybackStatus;

const DEFAULT_RATE: f64 = 1.0;

pub struct PlaybackClock {
    position: Duration,
    synced_at: Instant,
    rate: f64,
    status: PlaybackStatus,
}

impl PlaybackClock {
    pub fn new(position: Duration, status: PlaybackStatus, rate: f64) -> Self {
        Self {
            position,
            synced_at: Instant::now(),
            rate: sanitize_rate(rate),
            status,
        }
    }

    /// Current estimated playback position.
    ///
    /// While playing, the position advances using our local monotonic
    /// clock instead of repeatedly querying D-Bus for every lyric frame.
    pub fn position(&self) -> Duration {
        if self.status != PlaybackStatus::Playing {
            return self.position;
        }

        let elapsed = self.synced_at.elapsed().as_secs_f64();
        let advanced = elapsed * self.rate;

        self.position.saturating_add(duration_from_secs(advanced))
    }

    /// Hard synchronization against Spotify/MPRIS.
    pub fn sync(&mut self, position: Duration, status: PlaybackStatus, rate: f64) {
        self.position = position;
        self.synced_at = Instant::now();
        self.status = status;
        self.rate = sanitize_rate(rate);
    }

    /// Change playback state without changing the current position.
    pub fn set_status(&mut self, status: PlaybackStatus) {
        let current = self.position();

        self.position = current;
        self.synced_at = Instant::now();
        self.status = status;
    }

    pub fn set_rate(&mut self, rate: f64) {
        let current = self.position();

        self.position = current;
        self.synced_at = Instant::now();
        self.rate = sanitize_rate(rate);
    }

    pub fn status(&self) -> PlaybackStatus {
        self.status
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }
}

fn sanitize_rate(rate: f64) -> f64 {
    if rate.is_finite() && rate > 0.0 {
        rate
    } else {
        DEFAULT_RATE
    }
}

fn duration_from_secs(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds.max(0.0))
}
