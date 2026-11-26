use std::io;
use std::time::Duration;

use super::super::ExecReport;
use super::ErrorPipe;

#[test]
fn error_pipe_distinguishes_report_close_and_timeout() {
    let (reader, writer) = ErrorPipe::new().unwrap().into_parts();
    writer.report(3, libc::EACCES).unwrap();
    assert_eq!(
        reader.read(Duration::from_secs(1)).unwrap(),
        ExecReport::Failure {
            stage: 3,
            errno: libc::EACCES
        }
    );
    let (reader, writer) = ErrorPipe::new().unwrap().into_parts();
    drop(writer);
    assert_eq!(
        reader.read(Duration::from_secs(1)).unwrap(),
        ExecReport::ClosedWithoutReport
    );
    let (reader, _writer) = ErrorPipe::new().unwrap().into_parts();
    assert_eq!(
        reader.read(Duration::from_millis(10)).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
}
