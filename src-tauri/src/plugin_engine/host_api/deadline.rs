use super::*;

pub(super) const MIN_BLOCKING_TIMEOUT: Duration = Duration::from_millis(1);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProbeDeadline {
    expires_at: Option<Instant>,
}

impl ProbeDeadline {
    #[cfg(test)]
    pub(crate) fn none() -> Self {
        Self { expires_at: None }
    }

    pub(crate) fn at(expires_at: Instant) -> Self {
        Self {
            expires_at: Some(expires_at),
        }
    }

    pub(crate) fn has_elapsed(self) -> bool {
        self.expires_at
            .map(|expires_at| Instant::now() >= expires_at)
            .unwrap_or(false)
    }

    pub(super) fn clamp_duration(self, requested: Duration) -> Option<Duration> {
        let Some(expires_at) = self.expires_at else {
            return Some(requested);
        };
        let remaining = expires_at
            .checked_duration_since(Instant::now())
            .filter(|remaining| *remaining >= MIN_BLOCKING_TIMEOUT)?;
        Some(requested.min(remaining))
    }
}

pub(super) fn log_probe_deadline_skip(plugin_id: &str, operation: &str) {
    log::warn!(
        "[plugin:{}] {} skipped: probe timed out",
        plugin_id,
        operation
    );
}

pub(super) fn probe_timeout_error<'js>(ctx: &Ctx<'js>) -> rquickjs::Error {
    Exception::throw_message(ctx, "probe timed out")
}
