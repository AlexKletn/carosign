use super::{OBSERVERS_MAX, Observer, ObserverError};

/// Observers of one operation.
#[derive(Debug, Default)]
pub struct Observers {
    observers: Vec<Observer>,
}

impl Observers {
    /// Signing.
    ///
    /// ⚠️ Beyond the limit is failure, not quiet expansion.
    pub fn subscribe(&mut self, observer: Observer) -> Result<(), ObserverError> {
        if self.observers.len() >= OBSERVERS_MAX {
            return Err(ObserverError::TooMany);
        }
        if self.observers.iter().any(|item| item.id == observer.id) {
            return Err(ObserverError::Duplicate);
        }
        self.observers.push(observer);
        Ok(())
    }

    /// He's unsubscribed, he's returning if he was.
    pub fn unsubscribe(&mut self, id: u32) -> bool {
        let before = self.observers.len();
        self.observers.retain(|observer| observer.id != id);
        before != self.observers.len()
    }

    /// Who's waiting?
    pub fn list(&self) -> &[Observer] {
        &self.observers
    }

    pub fn len(&self) -> usize {
        self.observers.len()
    }

    /// It's empty.
    pub fn is_empty(&self) -> bool {
        self.observers.is_empty()
    }
}

#[cfg(test)]
#[path = "observers.tests.rs"]
mod tests;
