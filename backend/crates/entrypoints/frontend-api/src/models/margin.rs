#[expect(
    unused_imports,
    reason = "移行した model の domain 型 alias を保持するため"
)]
pub use core_domain::margin::{MarginAlertRecord, MarginInterestRecord, PubReason};
