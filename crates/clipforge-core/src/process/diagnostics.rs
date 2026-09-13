use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// Bounded diagnostic history. Poison recovery keeps error reporting available
/// even if another task failed while holding the history lock.
#[derive(Clone, Default)]
pub(crate) struct Diagnostics(Arc<Mutex<VecDeque<String>>>);

impl Diagnostics {
    pub fn push(&self, line: impl Into<String>) {
        let mut history = match self.0.lock() {
            Ok(history) => history,
            Err(poisoned) => poisoned.into_inner(),
        };
        if history.len() >= 30 {
            history.pop_front();
        }
        history.push_back(line.into());
    }

    pub fn detail(&self) -> String {
        let history = match self.0.lock() {
            Ok(history) => history,
            Err(poisoned) => poisoned.into_inner(),
        };
        history.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_history_remains_readable_and_writable() {
        let history = Diagnostics::default();
        let shared = history.0.clone();
        let _ = std::thread::spawn(move || {
            let _guard = shared.lock().unwrap();
            panic!("simulate failed writer");
        })
        .join();
        history.push("recovered");
        assert_eq!(history.detail(), "recovered");
    }

    #[test]
    fn retains_only_the_last_thirty_lines() {
        let history = Diagnostics::default();
        for i in 0..100 {
            history.push(i.to_string());
        }
        assert_eq!(history.detail().lines().count(), 30);
        assert!(history.detail().starts_with("70\n"));
    }
}
