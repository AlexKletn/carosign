mod pending_permit;
mod stage_permit;
mod switch_permit;

pub(super) use pending_permit::PendingPermit;
#[cfg(test)]
pub(super) use pending_permit::pending_fixture;
pub(super) use stage_permit::StagePermit;
#[cfg(test)]
pub(super) use stage_permit::stage_fixture;
pub(super) use switch_permit::SwitchPermit;
#[cfg(test)]
pub(super) use switch_permit::fixture;
