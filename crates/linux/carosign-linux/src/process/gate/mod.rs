//! Limited synchronization with the launch assistant.
//!
//! Channels do not confirm isolation, origin of the process, or successful exec.
//! They are only used by a separate assistant; there is no fork or callback after it.

#![allow(dead_code)]

mod admission_error;
mod admission_gate;
mod admission_state;
mod error_pipe;
mod error_reader;
mod error_writer;
mod exec_report;
mod gate_channel;
mod gate_receiver;
mod gate_sender;
mod policy_admission_error;
mod policy_admission_state;
mod policy_admissions;
mod read_until;

pub(crate) use admission_error::AdmissionError;
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use admission_gate::AdmissionGate;
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use error_pipe::ErrorPipe;
pub(crate) use error_reader::ErrorReader;
pub(crate) use error_writer::ErrorWriter;
pub(crate) use exec_report::ExecReport;
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use gate_channel::Gate;
pub(crate) use gate_receiver::GateReceiver;
pub(crate) use gate_sender::GateSender;
pub(crate) use policy_admission_error::PolicyAdmissionError;
pub(crate) use policy_admissions::PolicyAdmissions;

#[cfg(test)]
mod child_guard;

#[cfg(test)]
#[path = "gate_channel.tests.rs"]
mod tests;
