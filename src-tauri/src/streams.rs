//! Registro de los flujos abiertos (logs, estadísticas, terminales).
//!
//! Cada flujo tiene un identificador que la interfaz usa para cerrarlo o, en el
//! caso de la terminal, para enviarle entrada. Al terminar, el propio flujo se
//! retira del registro.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use dcc_engine::streams::TerminalInput;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Ctl {
    cancel: CancellationToken,
    input: Option<mpsc::Sender<TerminalInput>>,
}

#[derive(Default)]
pub struct StreamRegistry {
    next: AtomicU64,
    map: Mutex<HashMap<u64, Ctl>>,
}

impl StreamRegistry {
    pub fn register(&self, input: Option<mpsc::Sender<TerminalInput>>) -> (u64, CancellationToken) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = CancellationToken::new();
        self.map.lock().insert(
            id,
            Ctl {
                cancel: cancel.clone(),
                input,
            },
        );
        (id, cancel)
    }

    /// Pide el cierre del flujo. Devuelve `false` si ya no existía.
    pub fn close(&self, id: u64) -> bool {
        match self.map.lock().remove(&id) {
            Some(ctl) => {
                ctl.cancel.cancel();
                true
            }
            None => false,
        }
    }

    pub fn remove(&self, id: u64) {
        self.map.lock().remove(&id);
    }

    pub fn input(&self, id: u64) -> Option<mpsc::Sender<TerminalInput>> {
        self.map.lock().get(&id).and_then(|c| c.input.clone())
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.lock().len()
    }

    /// Cierra todos los flujos (al cerrar la ventana).
    pub fn close_all(&self) {
        for (_, ctl) in self.map.lock().drain() {
            ctl.cancel.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_close_and_input() {
        let r = StreamRegistry::default();
        let (tx, _rx) = mpsc::channel(1);
        let (a, ca) = r.register(None);
        let (b, cb) = r.register(Some(tx));
        assert_ne!(a, b);
        assert!(r.input(a).is_none());
        assert!(r.input(b).is_some());
        assert!(r.close(a));
        assert!(ca.is_cancelled());
        assert!(!cb.is_cancelled());
        assert!(!r.close(a), "cerrar dos veces no falla");
        r.close_all();
        assert!(cb.is_cancelled());
        assert_eq!(r.len(), 0);
    }
}
