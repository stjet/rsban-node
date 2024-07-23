use super::RepresentativeRegister;
use crate::stats::Stats;
use rsnano_core::Amount;
use rsnano_ledger::RepWeightCache;
use std::{sync::Arc, time::Duration};

pub const DEFAULT_ONLINE_WEIGHT_MINIMUM: Amount = Amount::nano(60_000_000);

pub struct RepresentativeRegisterBuilder {
    stats: Option<Arc<Stats>>,
    rep_weights: Option<Arc<RepWeightCache>>,
    weight_period: Duration,
    online_weight_minimum: Amount,
    trended: Option<Amount>,
}

impl RepresentativeRegisterBuilder {
    pub(super) fn new() -> Self {
        Self {
            stats: None,
            rep_weights: None,
            weight_period: Duration::from_secs(5 * 60),
            online_weight_minimum: DEFAULT_ONLINE_WEIGHT_MINIMUM,
            trended: None,
        }
    }
    pub fn stats(mut self, stats: Arc<Stats>) -> Self {
        self.stats = Some(stats);
        self
    }

    pub fn rep_weights(mut self, weights: Arc<RepWeightCache>) -> Self {
        self.rep_weights = Some(weights);
        self
    }

    pub fn weight_period(mut self, period: Duration) -> Self {
        self.weight_period = period;
        self
    }

    pub fn online_weight_minimum(mut self, minimum: Amount) -> Self {
        self.online_weight_minimum = minimum;
        self
    }

    pub fn trended(mut self, trended: Amount) -> Self {
        self.trended = Some(trended);
        self
    }

    pub fn finish(self) -> RepresentativeRegister {
        let stats = self.stats.unwrap_or_else(|| Arc::new(Stats::default()));
        let rep_weights = self
            .rep_weights
            .unwrap_or_else(|| Arc::new(RepWeightCache::new()));

        let mut register = RepresentativeRegister::new(
            rep_weights,
            stats,
            self.weight_period,
            self.online_weight_minimum,
        );
        if let Some(trended) = self.trended {
            register.set_trended(trended);
        }
        register
    }
}
