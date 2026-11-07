//! Idempotency for `receipts_ingest` (contract §4.1, §4.2): a batch key maps to the
//! parameter digest and the result it produced, under the active recovery epoch.

use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use airs_runtime_core::dto::ErrorCode;

use crate::control::{Control, Refused};

fn refused(code: ErrorCode, reason: &'static str) -> Refused {
    Refused {
        code,
        reason,
        retry_after_seconds: None,
    }
}

fn storage(_: rusqlite::Error) -> Refused {
    refused(ErrorCode::StorageUnavailable, "control store unavailable")
}

/// What a batch key already recorded.
#[derive(Debug, PartialEq)]
pub enum Prior {
    /// Never seen under the active epoch.
    New,
    /// The same parameters: the stored result.
    Same(Value),
    /// The same parameters, reserved but not finished (a crash after the reservation):
    /// the caller repeats its idempotent effects, in the archive run the reservation
    /// bound (`None` only for a reservation an earlier build wrote, before
    /// `ingest_reserve` migrates it), and records the result.
    Pending(Option<String>),
}

impl Control {
    /// Open an atomic unit (a savepoint, so control methods using savepoints nest).
    pub fn begin_unit(&mut self) -> Result<(), Refused> {
        self.conn.execute_batch("SAVEPOINT unit").map_err(storage)
    }

    /// Commit (`keep`) or roll back the unit opened by [`Control::begin_unit`].
    pub fn end_unit(&mut self, keep: bool) -> Result<(), Refused> {
        let sql = if keep {
            "RELEASE unit"
        } else {
            "ROLLBACK TO unit; RELEASE unit"
        };
        self.conn.execute_batch(sql).map_err(storage)
    }

    /// Check a batch key under the active epoch: `control_restored` for another epoch,
    /// `idempotency_conflict` for the same key with different parameters.
    pub fn ingest_prior(
        &self,
        tenant: &str,
        (sub, client): (&str, &str),
        expected_epoch: &str,
        key: &str,
        digest: &str,
    ) -> Result<Prior, Refused> {
        let (epoch, _) = self.active_epoch()?;
        if epoch != expected_epoch {
            return Err(refused(
                ErrorCode::ControlRestored,
                "the idempotency key was minted under another recovery epoch",
            ));
        }
        let row: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT digest, result_json FROM ingest_results WHERE tenant=?1 AND sub=?2 AND client_id=?3 AND epoch=?4 AND key=?5",
                params![tenant, sub, client, epoch, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        match row {
            None => Ok(Prior::New),
            Some((d, _)) if d != digest => Err(refused(
                ErrorCode::IdempotencyConflict,
                "the idempotency key was used with different parameters",
            )),
            Some((_, result)) => {
                let v: Value = serde_json::from_str(&result).map_err(|_| {
                    refused(ErrorCode::StorageUnavailable, "stored result is invalid")
                })?;
                Ok(if v.get("pending").is_some() {
                    Prior::Pending(v["run"]["id"].as_str().map(str::to_owned))
                } else {
                    Prior::Same(v)
                })
            }
        }
    }

    /// Reserve a batch key before any effect (M4 review 1 finding 7): the first
    /// reservation of (caller, epoch, key) binds its digest, so different parameters under
    /// that key are refused at once, before anything is written. The reservation also
    /// binds the batch's archive run (`run_id`, identity format 2) in the same write.
    /// An existing pending reservation without a binding (written by an earlier build) is
    /// returned unbound: the caller resolves its ownership from proven provenance and
    /// binds it with [`Control::ingest_bind`] before any effect.
    #[allow(clippy::too_many_arguments)]
    pub fn ingest_reserve(
        &mut self,
        tenant: &str,
        (sub, client): (&str, &str),
        epoch: &str,
        key: &str,
        digest: &str,
        run_id: &str,
        now_ms: i64,
    ) -> Result<Prior, Refused> {
        let (active, _) = self.active_epoch()?;
        if active != epoch {
            return Err(refused(
                ErrorCode::ControlRestored,
                "the idempotency key was minted under another recovery epoch",
            ));
        }
        let pending = serde_json::json!({"pending": true, "run": {"id": run_id, "format": 2}});
        self.conn
            .execute(
                "INSERT INTO ingest_results VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT DO NOTHING",
                params![tenant, sub, client, epoch, key, digest, pending.to_string(), now_ms],
            )
            .map_err(storage)?;
        self.ingest_prior(tenant, (sub, client), epoch, key, digest)
    }

    /// Bind an unbound pending reservation to `run_id` (identity `format`), durably,
    /// before any archive effect; a reservation already bound keeps its binding.
    #[allow(clippy::too_many_arguments)]
    pub fn ingest_bind(
        &mut self,
        tenant: &str,
        (sub, client): (&str, &str),
        epoch: &str,
        key: &str,
        digest: &str,
        run_id: &str,
        format: u8,
    ) -> Result<Prior, Refused> {
        let bound = serde_json::json!({"pending": true, "run": {"id": run_id, "format": format}});
        self.conn
            .execute(
                "UPDATE ingest_results SET result_json=?7 WHERE tenant=?1 AND sub=?2 AND client_id=?3 AND epoch=?4 AND key=?5 AND digest=?6
                 AND json_extract(result_json,'$.pending') IS NOT NULL AND json_extract(result_json,'$.run.id') IS NULL",
                params![tenant, sub, client, epoch, key, digest, bound.to_string()],
            )
            .map_err(storage)?;
        self.ingest_prior(tenant, (sub, client), epoch, key, digest)
    }

    /// How many recovery epochs this store has ever had (1: no restore ever happened).
    pub fn epoch_count(&self) -> Result<i64, Refused> {
        self.conn
            .query_row("SELECT count(*) FROM epochs", [], |r| r.get(0))
            .map_err(storage)
    }

    #[allow(clippy::too_many_arguments)]
    /// Record a batch's result (first writer wins; a concurrent duplicate keeps the
    /// stored one, which is returned).
    pub fn ingest_record(
        &mut self,
        tenant: &str,
        (sub, client): (&str, &str),
        epoch: &str,
        key: &str,
        digest: &str,
        result: &Value,
        now_ms: i64,
    ) -> Result<Value, Refused> {
        self.conn
            .execute(
                "INSERT INTO ingest_results VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(tenant, sub, client_id, epoch, key) DO UPDATE SET result_json=excluded.result_json WHERE ingest_results.digest=excluded.digest AND json_extract(ingest_results.result_json,'$.pending') IS NOT NULL",
                params![tenant, sub, client, epoch, key, digest, result.to_string(), now_ms],
            )
            .map_err(storage)?;
        match self.ingest_prior(tenant, (sub, client), epoch, key, digest)? {
            Prior::Same(v) => Ok(v),
            Prior::New | Prior::Pending(_) => Err(storage(rusqlite::Error::QueryReturnedNoRows)),
        }
    }
}
