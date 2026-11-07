//! M5 exit gate: the B4 adversarial transfer and prune probes, against the server-owned
//! store through the test driver (the CLI no longer owns SQLite).
#[path = "support/history_transfer_adversarial.rs"]
mod adversarial;
#[path = "support/transfer_cases.rs"]
mod transfer_cases;
