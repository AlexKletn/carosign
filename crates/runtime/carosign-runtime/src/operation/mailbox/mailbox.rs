use super::{CONTROL_BURST, CONTROL_CAPACITY, MailboxFull, NORMAL_CAPACITY, Priority};

/// Frozen queue.
#[derive(Debug)]
pub struct Mailbox<T> {
    control: std::collections::VecDeque<T>,
    normal: std::collections::VecDeque<T>,
    control_since_normal: usize,
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Mailbox<T> {
    /// Empty mail.
    pub fn new() -> Self {
        Self {
            control: std::collections::VecDeque::new(),
            normal: std::collections::VecDeque::new(),
            control_since_normal: 0,
        }
    }

    /// He's putting in a control message.
    ///
    /// ⚠️ Overflow is a rejection, not a throwout: no review can be lost.
    pub fn push_control(&mut self, message: T) -> Result<(), MailboxFull> {
        if self.control.len() >= CONTROL_CAPACITY {
            return Err(MailboxFull {
                priority: Priority::Control,
            });
        }
        self.control.push_back(message);
        Ok(())
    }

    /// He's posting a regular message.
    pub fn push_normal(&mut self, message: T) -> Result<(), MailboxFull> {
        if self.normal.len() >= NORMAL_CAPACITY {
            return Err(MailboxFull {
                priority: Priority::Normal,
            });
        }
        self.normal.push_back(message);
        Ok(())
    }

    /// Takes the following message: control forward, but the usual does not starve.
    ///
    /// ⚠️ After `CONTROL_BURST` One thing is taken by the management message - otherwise a
    /// continuous stream of feedback would stop the work forever.
    pub fn take(&mut self) -> Option<T> {
        if !self.control.is_empty() && self.control_since_normal < CONTROL_BURST {
            self.control_since_normal += 1;
            return self.control.pop_front();
        }
        if let Some(message) = self.normal.pop_front() {
            self.control_since_normal = 0;
            return Some(message);
        }
        if let Some(message) = self.control.pop_front() {
            self.control_since_normal = 1;
            return Some(message);
        }
        None
    }

    /// The actor's governing events can cancel any normal transition.
    /// While they are waiting, the usual message should not bypass them.
    pub(crate) fn take_control_first(&mut self) -> Option<T> {
        if let Some(message) = self.control.pop_front() {
            return Some(message);
        }
        self.control_since_normal = 0;
        self.normal.pop_front()
    }

    /// How long does it take to control?
    pub fn control_len(&self) -> usize {
        self.control.len()
    }

    /// How long does the usual wait?
    pub fn normal_len(&self) -> usize {
        self.normal.len()
    }

    /// If there's a pending review.
    pub fn has_control(&self) -> bool {
        !self.control.is_empty()
    }
}

#[path = "mailbox.tests.rs"]
#[cfg(test)]
mod tests;
