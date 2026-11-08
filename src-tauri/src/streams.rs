//! Registro de los flujos abiertos (logs, estadísticas, terminales).
//!
//! Cada flujo tiene un identificador que la interfaz usa para cerrarlo o, en el
//! caso de la terminal, para enviarle entrada. Al terminar, el propio flujo se
//! retira del registro.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

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
    /// Flujos cuya tarea aún no ha terminado (aunque se haya pedido cerrarlos):
    /// una terminal tarda un instante en salir y limpiar sus procesos (D-063).
    running: Mutex<HashSet<u64>>,
    finished: tokio::sync::Notify,
}

impl StreamRegistry {
    pub fn register(&self, input: Option<mpsc::Sender<TerminalInput>>) -> (u64, CancellationToken) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = CancellationToken::new();
        self.running.lock().insert(id);
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

    /// La tarea del flujo ha terminado.
    pub fn remove(&self, id: u64) {
        self.map.lock().remove(&id);
        self.running.lock().remove(&id);
        self.finished.notify_waiters();
    }

    /// Flujos cuya tarea sigue en marcha.
    pub fn running(&self) -> usize {
        self.running.lock().len()
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

    /// Cierra todos los flujos y espera, como mucho `timeout`, a que sus tareas
    /// terminen (las terminales envían Ctrl+C y Ctrl+D y limpian sus procesos).
    /// Devuelve cuántas quedaron sin terminar.
    pub async fn close_all_and_wait(&self, timeout: Duration) -> usize {
        self.close_all();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let notified = self.finished.notified();
            if self.running() == 0 {
                return 0;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return self.running();
            }
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

    #[tokio::test]
    async fn close_all_waits_for_the_tasks_to_finish() {
        let r = std::sync::Arc::new(StreamRegistry::default());
        let (a, ca) = r.register(None);
        let (b, cb) = r.register(None);
        // Dos tareas que, al cancelarse, tardan en terminar (como una terminal).
        for (id, c, ms) in [(a, ca, 150u64), (b, cb, 300)] {
            let r2 = r.clone();
            tokio::spawn(async move {
                c.cancelled().await;
                tokio::time::sleep(Duration::from_millis(ms)).await;
                r2.remove(id);
            });
        }
        assert_eq!(r.running(), 2);
        let t = std::time::Instant::now();
        assert_eq!(r.close_all_and_wait(Duration::from_secs(5)).await, 0);
        assert!(
            t.elapsed() >= Duration::from_millis(300),
            "esperó a las dos"
        );
        // Una que no termina: se devuelve al agotar el plazo.
        let (_c, _cc) = r.register(None);
        assert_eq!(r.close_all_and_wait(Duration::from_millis(100)).await, 1);
    }
}
