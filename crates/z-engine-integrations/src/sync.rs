//! Lock access that survives poisoning. Every critical section in this crate
//! only inserts, removes or copies plain values, so the protected data stays
//! consistent even when a holder panicked; continuing with it is correct,
//! unlike pretending the state is empty.

use std::sync::{Mutex, MutexGuard, PoisonError};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
