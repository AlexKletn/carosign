use super::*;

pub(super) enum ClientRun {
    Thread(std::thread::JoinHandle<()>),
    Process(MockChild),
}
