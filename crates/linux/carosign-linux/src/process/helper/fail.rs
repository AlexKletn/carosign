use super::super::gate::ErrorWriter;
use super::HelperFailure;
use super::HelperStage;

pub(super) fn terminate(report: ErrorWriter, stage: HelperStage, errno: i32) -> ! {
    let _ = report.report(stage.wire(), errno);
    // SAFETY: Helper is a separate process; after irreversible steps, no return is allowed.
    unsafe { libc::_exit(126) }
}

pub(super) fn fail(report: ErrorWriter, stage: HelperStage, errno: i32) -> HelperFailure {
    let report_failed = report.report(stage.wire(), errno).is_err();
    HelperFailure {
        stage,
        errno,
        report_failed,
    }
}
