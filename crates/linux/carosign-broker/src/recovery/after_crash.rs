use core::fmt;

use carosign_core::ids::LeaseId;

/// What to do after a mediator falls?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AfterCrash {
    /// All the renditions are unclear.
    AllUncertain,
    /// Access is closed by the kernel; the state of the device is not confirmed by this.
    KernelClosed,
}

/// The decision to fall.
///
/// `kernel_enforced` requires kernel evidence. Without it, the result is `AllUncertain`.
pub fn after_crash(kernel_enforced: bool) -> AfterCrash {
    if kernel_enforced {
        AfterCrash::KernelClosed
    } else {
        AfterCrash::AllUncertain
    }
}

/// Leasings that have become unclear.
///
/// Closing the descriptors does not prove the completion of the hardware call.
/// This interface does not receive a separate certificate of the device.
pub fn uncertain_leases(leases: &[LeaseId], after: AfterCrash) -> Vec<LeaseId> {
    match after {
        AfterCrash::AllUncertain | AfterCrash::KernelClosed => leases.to_vec(),
    }
}

impl fmt::Display for AfterCrash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AllUncertain => "все выдачи неясны",
            Self::KernelClosed => "доступ закрыт ядром",
        })
    }
}
