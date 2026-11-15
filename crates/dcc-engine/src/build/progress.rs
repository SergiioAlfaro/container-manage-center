// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Progreso de la construcción.
//!
//! * BuildKit (`version=2`) envía mensajes `moby.buildkit.trace` cuyo `aux` es un
//!   `StatusResponse` de BuildKit en protobuf, codificado en base64. Se
//!   decodifica con un lector mínimo de protobuf (solo los campos necesarios de
//!   `api/services/control/control.proto`) para no depender de gRPC.
//! * El constructor clásico (`version=1`) envía texto en `stream`.
//!
//! Ambos se convierten en pasos y en líneas de texto con el formato de
//! `docker build --progress=plain`.

use std::collections::HashMap;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ------------------------------------------------------------- protobuf mínimo

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

enum Field<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Other,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, i: 0 }
    }
    fn varint(&mut self) -> Option<u64> {
        let mut v: u64 = 0;
        for shift in (0..64).step_by(7) {
            let byte = *self.b.get(self.i)?;
            self.i += 1;
            v |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    /// Siguiente campo: (número, valor). `None` al final o si los datos están mal.
    fn next(&mut self) -> Option<(u64, Field<'a>)> {
        if self.i >= self.b.len() {
            return None;
        }
        let key = self.varint()?;
        let (num, wire) = (key >> 3, key & 7);
        let f = match wire {
            0 => Field::Varint(self.varint()?),
            1 => {
                self.i = self.i.checked_add(8).filter(|&n| n <= self.b.len())?;
                Field::Other
            }
            2 => {
                let len = usize::try_from(self.varint()?).ok()?;
                let end = self.i.checked_add(len).filter(|&n| n <= self.b.len())?;
                let s = &self.b[self.i..end];
                self.i = end;
                Field::Bytes(s)
            }
            5 => {
                self.i = self.i.checked_add(4).filter(|&n| n <= self.b.len())?;
                Field::Other
            }
            _ => return None,
        };
        Some((num, f))
    }
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `google.protobuf.Timestamp` → nanosegundos desde la época.
fn timestamp(b: &[u8]) -> i128 {
    let mut r = Reader::new(b);
    let (mut secs, mut nanos) = (0i128, 0i128);
    while let Some((n, f)) = r.next() {
        if let Field::Varint(v) = f {
            match n {
                1 => secs = v as i64 as i128,
                2 => nanos = v as i64 as i128,
                _ => {}
            }
        }
    }
    secs * 1_000_000_000 + nanos
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PbVertex {
    pub digest: String,
    pub name: String,
    pub cached: bool,
    pub started: Option<i128>,
    pub completed: Option<i128>,
    pub error: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PbStatus {
    pub id: String,
    pub vertex: String,
    pub name: String,
    pub current: i64,
    pub total: i64,
    pub completed: bool,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PbLog {
    pub vertex: String,
    pub stream: i64,
    pub msg: Vec<u8>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PbStatusResponse {
    pub vertexes: Vec<PbVertex>,
    pub statuses: Vec<PbStatus>,
    pub logs: Vec<PbLog>,
    pub warnings: Vec<(String, String)>,
}

/// Decodifica un `moby.buildkit.v1.StatusResponse`.
pub fn decode_status(b: &[u8]) -> Option<PbStatusResponse> {
    let mut out = PbStatusResponse::default();
    let mut r = Reader::new(b);
    while let Some((n, f)) = r.next() {
        let Field::Bytes(m) = f else { continue };
        match n {
            1 => {
                let mut v = PbVertex::default();
                let mut r = Reader::new(m);
                while let Some((n, f)) = r.next() {
                    match (n, f) {
                        (1, Field::Bytes(x)) => v.digest = s(x),
                        (3, Field::Bytes(x)) => v.name = s(x),
                        (4, Field::Varint(x)) => v.cached = x != 0,
                        (5, Field::Bytes(x)) => v.started = Some(timestamp(x)),
                        (6, Field::Bytes(x)) => v.completed = Some(timestamp(x)),
                        (7, Field::Bytes(x)) => v.error = s(x),
                        _ => {}
                    }
                }
                out.vertexes.push(v);
            }
            2 => {
                let mut v = PbStatus::default();
                let mut r = Reader::new(m);
                while let Some((n, f)) = r.next() {
                    match (n, f) {
                        (1, Field::Bytes(x)) => v.id = s(x),
                        (2, Field::Bytes(x)) => v.vertex = s(x),
                        (3, Field::Bytes(x)) => v.name = s(x),
                        (4, Field::Varint(x)) => v.current = x as i64,
                        (5, Field::Varint(x)) => v.total = x as i64,
                        (8, Field::Bytes(_)) => v.completed = true,
                        _ => {}
                    }
                }
                out.statuses.push(v);
            }
            3 => {
                let mut v = PbLog::default();
                let mut r = Reader::new(m);
                while let Some((n, f)) = r.next() {
                    match (n, f) {
                        (1, Field::Bytes(x)) => v.vertex = s(x),
                        (3, Field::Varint(x)) => v.stream = x as i64,
                        (4, Field::Bytes(x)) => v.msg = x.to_vec(),
                        _ => {}
                    }
                }
                out.logs.push(v);
            }
            4 => {
                let (mut vertex, mut short) = (String::new(), String::new());
                let mut r = Reader::new(m);
                while let Some((n, f)) = r.next() {
                    match (n, f) {
                        (1, Field::Bytes(x)) => vertex = s(x),
                        (3, Field::Bytes(x)) => short = s(x),
                        _ => {}
                    }
                }
                out.warnings.push((vertex, short));
            }
            _ => {}
        }
    }
    Some(out)
}

// ------------------------------------------------------------- modelo común

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StepState {
    Running,
    Done,
    Cached,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildStep {
    /// Número de paso (`#N` en la salida).
    pub number: u32,
    pub name: String,
    pub state: StepState,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildUpdate {
    /// Pasos nuevos o que cambiaron.
    pub steps: Vec<BuildStep>,
    /// Líneas nuevas de la salida en formato `--progress=plain`.
    pub lines: Vec<String>,
}

/// Convierte los mensajes de `/build` en pasos y líneas.
#[derive(Debug, Default)]
pub struct BuildTracker {
    steps: Vec<BuildStep>,
    by_digest: HashMap<String, usize>,
    started_at: HashMap<usize, i128>,
    changed: Vec<usize>,
    lines: Vec<String>,
    /// Texto pendiente de una línea a medias del constructor clásico.
    partial: String,
    pub image_id: Option<String>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    /// Pasos que BuildKit tomó de la caché.
    pub cached: u32,
}

impl BuildTracker {
    fn mark(&mut self, i: usize) {
        if !self.changed.contains(&i) {
            self.changed.push(i);
        }
    }

    fn step_for(&mut self, digest: &str, name: &str) -> usize {
        if let Some(&i) = self.by_digest.get(digest) {
            if !name.is_empty() && self.steps[i].name.is_empty() {
                self.steps[i].name = name.to_owned();
            }
            return i;
        }
        let i = self.steps.len();
        self.steps.push(BuildStep {
            number: i as u32 + 1,
            name: name.to_owned(),
            state: StepState::Running,
            duration_ms: None,
            error: None,
        });
        self.by_digest.insert(digest.to_owned(), i);
        self.lines.push(format!("#{} {name}", i + 1));
        self.mark(i);
        i
    }

    /// Procesa un mensaje JSON de la respuesta de `/build`.
    pub fn push_json(&mut self, v: &serde_json::Value) {
        if let Some(e) = v
            .get("errorDetail")
            .and_then(|d| d.get("message"))
            .and_then(|m| m.as_str())
            .or_else(|| v.get("error").and_then(|m| m.as_str()))
        {
            self.error = Some(e.trim().to_owned());
            self.lines.push(format!("ERROR: {}", e.trim()));
            return;
        }
        let id = v.get("id").and_then(|i| i.as_str());
        match (id, v.get("aux")) {
            (Some("moby.buildkit.trace"), Some(serde_json::Value::String(b64))) => {
                if let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(b64) {
                    if let Some(st) = decode_status(&raw) {
                        self.push_buildkit(st);
                    }
                }
                return;
            }
            (Some("moby.image.id"), Some(aux)) | (None, Some(aux)) => {
                if let Some(i) = aux.get("ID").and_then(|i| i.as_str()) {
                    self.image_id = Some(i.to_owned());
                }
                return;
            }
            _ => {}
        }
        if let Some(text) = v.get("stream").and_then(|s| s.as_str()) {
            self.push_classic(text);
        } else if let Some(status) = v.get("status").and_then(|s| s.as_str()) {
            // Descarga de la imagen base en el constructor clásico.
            let line = match id {
                Some(i) => format!("{i}: {status}"),
                None => status.to_owned(),
            };
            if v.get("progressDetail")
                .and_then(|p| p.get("current"))
                .is_none()
            {
                self.lines.push(line);
            }
        }
    }

    pub fn push_buildkit(&mut self, st: PbStatusResponse) {
        for v in st.vertexes {
            let i = self.step_for(&v.digest, &v.name);
            if let Some(t) = v.started {
                self.started_at.entry(i).or_insert(t);
            }
            let prev = self.steps[i].state;
            if !v.error.is_empty() {
                self.steps[i].state = StepState::Error;
                self.steps[i].error = Some(v.error.clone());
                if prev != StepState::Error {
                    self.lines.push(format!("#{} ERROR: {}", i + 1, v.error));
                }
            } else if v.cached {
                if prev != StepState::Cached {
                    self.steps[i].state = StepState::Cached;
                    self.cached += 1;
                    self.lines.push(format!("#{} CACHED", i + 1));
                }
            } else if let Some(end) = v.completed {
                if prev == StepState::Running {
                    self.steps[i].state = StepState::Done;
                    let start = self.started_at.get(&i).copied().unwrap_or(end);
                    let ms = ((end - start).max(0) / 1_000_000) as u64;
                    self.steps[i].duration_ms = Some(ms);
                    self.lines
                        .push(format!("#{} DONE {:.1}s", i + 1, ms as f64 / 1000.0));
                }
            }
            self.mark(i);
        }
        for s in st.statuses {
            if !s.completed {
                continue;
            }
            if let Some(&i) = self.by_digest.get(&s.vertex) {
                let what = if s.name.is_empty() {
                    s.id.clone()
                } else {
                    s.name.clone()
                };
                let amount = if s.total > 0 {
                    format!(" {}/{}", s.current, s.total)
                } else if s.current > 0 {
                    format!(" {}", s.current)
                } else {
                    String::new()
                };
                self.lines.push(format!("#{} {what}{amount} done", i + 1));
            }
        }
        for l in st.logs {
            let n = self.by_digest.get(&l.vertex).map(|i| i + 1).unwrap_or(0);
            let text = String::from_utf8_lossy(&l.msg);
            for line in text.lines() {
                self.lines.push(format!("#{n} {line}"));
            }
        }
        for (_, w) in st.warnings {
            self.warnings.push(w.clone());
            self.lines.push(format!("WARNING: {w}"));
        }
    }

    fn push_classic(&mut self, text: &str) {
        self.partial.push_str(text);
        while let Some(i) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=i).collect();
            let line = line.trim_end_matches(['\n', '\r']).to_owned();
            self.classic_line(line);
        }
        // "Step 1/4 : FROM x" llega sin salto de línea: se procesa ya.
        if self.partial.starts_with("Step ") {
            let line = std::mem::take(&mut self.partial);
            self.classic_line(line);
        }
    }

    fn classic_line(&mut self, line: String) {
        if line.is_empty() {
            return;
        }
        if let Some(rest) = line.strip_prefix("Step ") {
            if let Some((_, instr)) = rest.split_once(" : ") {
                // Cierra el paso anterior.
                if let Some(last) = self.steps.last_mut() {
                    if last.state == StepState::Running {
                        last.state = StepState::Done;
                        let i = self.steps.len() - 1;
                        self.mark(i);
                    }
                }
                let key = format!("classic-{}", self.steps.len());
                self.step_for(&key, instr);
                return;
            }
        }
        let n = self.steps.len();
        if line.trim() == "---> Using cache" {
            if let Some(last) = self.steps.last_mut() {
                last.state = StepState::Cached;
                self.cached += 1;
                self.mark(n - 1);
            }
        }
        self.lines
            .push(if n > 0 { format!("#{n} {line}") } else { line });
    }

    /// Cierra el último paso del constructor clásico al terminar bien.
    pub fn finish(&mut self) {
        if !self.partial.is_empty() {
            let line = std::mem::take(&mut self.partial);
            self.classic_line(line);
        }
        if self.error.is_none() {
            for i in 0..self.steps.len() {
                if self.steps[i].state == StepState::Running
                    && self.by_digest.keys().any(|k| k.starts_with("classic-"))
                {
                    self.steps[i].state = StepState::Done;
                    self.mark(i);
                }
            }
        }
    }

    pub fn take_update(&mut self) -> Option<BuildUpdate> {
        if self.changed.is_empty() && self.lines.is_empty() {
            return None;
        }
        let steps = std::mem::take(&mut self.changed)
            .into_iter()
            .map(|i| self.steps[i].clone())
            .collect();
        Some(BuildUpdate {
            steps,
            lines: std::mem::take(&mut self.lines),
        })
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Codificador mínimo para construir mensajes de prueba.
    fn varint(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                break;
            }
            out.push(b | 0x80);
        }
    }
    fn field_bytes(n: u64, b: &[u8], out: &mut Vec<u8>) {
        varint(n << 3 | 2, out);
        varint(b.len() as u64, out);
        out.extend_from_slice(b);
    }
    fn field_varint(n: u64, v: u64, out: &mut Vec<u8>) {
        varint(n << 3, out);
        varint(v, out);
    }
    fn ts(secs: u64, nanos: u64) -> Vec<u8> {
        let mut o = Vec::new();
        field_varint(1, secs, &mut o);
        field_varint(2, nanos, &mut o);
        o
    }
    fn vertex(
        d: &str,
        name: &str,
        cached: bool,
        start: Option<u64>,
        end: Option<u64>,
        err: &str,
    ) -> Vec<u8> {
        let mut o = Vec::new();
        field_bytes(1, d.as_bytes(), &mut o);
        field_bytes(2, b"sha256:input", &mut o);
        field_bytes(3, name.as_bytes(), &mut o);
        if cached {
            field_varint(4, 1, &mut o);
        }
        if let Some(s) = start {
            field_bytes(5, &ts(s, 0), &mut o);
        }
        if let Some(e) = end {
            field_bytes(6, &ts(e, 500_000_000), &mut o);
        }
        if !err.is_empty() {
            field_bytes(7, err.as_bytes(), &mut o);
        }
        o
    }
    fn response(vs: &[Vec<u8>], logs: &[(&str, &str)]) -> String {
        let mut o = Vec::new();
        for v in vs {
            field_bytes(1, v, &mut o);
        }
        for (v, m) in logs {
            let mut l = Vec::new();
            field_bytes(1, v.as_bytes(), &mut l);
            field_bytes(2, &ts(1, 0), &mut l);
            field_varint(3, 1, &mut l);
            field_bytes(4, m.as_bytes(), &mut l);
            field_bytes(3, &l, &mut o);
        }
        base64::engine::general_purpose::STANDARD.encode(o)
    }

    fn trace(b64: String) -> serde_json::Value {
        serde_json::json!({"id": "moby.buildkit.trace", "aux": b64})
    }

    #[test]
    fn buildkit_trace_becomes_plain_progress() {
        let mut t = BuildTracker::default();
        t.push_json(&trace(response(
            &[vertex(
                "sha256:a",
                "[1/2] FROM base",
                false,
                Some(10),
                None,
                "",
            )],
            &[],
        )));
        t.push_json(&trace(response(
            &[vertex(
                "sha256:a",
                "[1/2] FROM base",
                true,
                Some(10),
                Some(10),
                "",
            )],
            &[],
        )));
        t.push_json(&trace(response(
            &[vertex(
                "sha256:b",
                "[2/2] RUN echo hola",
                false,
                Some(11),
                None,
                "",
            )],
            &[("sha256:b", "hola\nmundo\n")],
        )));
        t.push_json(&trace(response(
            &[vertex(
                "sha256:b",
                "[2/2] RUN echo hola",
                false,
                Some(11),
                Some(12),
                "",
            )],
            &[],
        )));
        t.push_json(&serde_json::json!({"id": "moby.image.id", "aux": {"ID": "sha256:feed"}}));
        let u = t.take_update().unwrap();
        assert_eq!(
            u.lines,
            vec![
                "#1 [1/2] FROM base",
                "#1 CACHED",
                "#2 [2/2] RUN echo hola",
                "#2 hola",
                "#2 mundo",
                "#2 DONE 1.5s"
            ]
        );
        assert_eq!(u.steps.len(), 2);
        assert_eq!(u.steps[0].state, StepState::Cached);
        assert_eq!(u.steps[1].state, StepState::Done);
        assert_eq!(u.steps[1].duration_ms, Some(1500));
        assert_eq!(t.image_id.as_deref(), Some("sha256:feed"));
        assert_eq!(t.cached, 1);
        assert!(t.take_update().is_none());
    }

    #[test]
    fn buildkit_errors_are_reported() {
        let mut t = BuildTracker::default();
        t.push_json(&trace(response(
            &[vertex(
                "sha256:c",
                "[2/2] RUN false",
                false,
                Some(1),
                Some(2),
                "process \"/bin/sh -c false\" did not complete successfully: exit code: 1",
            )],
            &[],
        )));
        t.push_json(&serde_json::json!({"errorDetail": {"message": "process did not complete successfully"}, "error": "process did not complete successfully"}));
        let u = t.take_update().unwrap();
        assert_eq!(u.steps[0].state, StepState::Error);
        assert!(u.lines.iter().any(|l| l.starts_with("#1 ERROR: process")));
        assert_eq!(
            t.error.as_deref(),
            Some("process did not complete successfully")
        );
    }

    #[test]
    fn real_daemon_trace_decodes() {
        // Mensaje capturado de Docker 29 (BuildKit) para "[internal] load remote build context".
        let v = serde_json::json!({"id":"moby.buildkit.trace","aux":"Cm8KR3NoYTI1NjphYmU3MGRkM2Y1MTcyYTkwMDIzMzA3Y2M0MmNkOWFhMGRjMDEzNzEyNjY1YjQ5YjVkMzc5OTk1MDJlOTdhMzVhGiRbaW50ZXJuYWxdIGxvYWQgcmVtb3RlIGJ1aWxkIGNvbnRleHQ="});
        let mut t = BuildTracker::default();
        t.push_json(&v);
        let u = t.take_update().unwrap();
        assert_eq!(u.lines, vec!["#1 [internal] load remote build context"]);
    }

    #[test]
    fn classic_builder_stream() {
        let mut t = BuildTracker::default();
        for m in [
            r#"{"stream":"Step 1/3 : FROM base"}"#,
            r#"{"stream":"\n"}"#,
            r#"{"stream":" ---> cfbe81364cdd\n"}"#,
            r#"{"stream":"Step 2/3 : COPY a /a"}"#,
            r#"{"stream":"\n"}"#,
            r#"{"stream":" ---> Using cache\n"}"#,
            r#"{"stream":"Step 3/3 : RUN echo hola"}"#,
            r#"{"stream":"\n"}"#,
            r#"{"stream":"hola\n"}"#,
            r#"{"aux":{"ID":"sha256:fb3d"}}"#,
            r#"{"stream":"Successfully built fb3db21b0e9b\n"}"#,
        ] {
            t.push_json(&serde_json::from_str(m).unwrap());
        }
        t.finish();
        let u = t.take_update().unwrap();
        assert_eq!(t.step_count(), 3);
        assert!(u.lines.contains(&"#1 FROM base".to_string()));
        assert!(u.lines.contains(&"#3 hola".to_string()));
        let last: HashMap<u32, StepState> = u.steps.iter().map(|s| (s.number, s.state)).collect();
        assert_eq!(last[&2], StepState::Cached);
        assert_eq!(last[&3], StepState::Done);
        assert_eq!(t.image_id.as_deref(), Some("sha256:fb3d"));
    }

    #[test]
    fn malformed_protobuf_does_not_panic() {
        assert!(decode_status(&[0xff, 0xff, 0xff]).is_some());
        let mut r = Reader::new(&[0x0a, 0x50, 0x01]);
        assert!(r.next().is_none(), "longitud mayor que los datos");
    }
}
