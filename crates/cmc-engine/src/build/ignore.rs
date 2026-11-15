// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! `.dockerignore` con la semántica de `moby/patternmatcher`, que es la que usa
//! el CLI de Docker al preparar el contexto:
//!
//! * Una línea por patrón; las vacías y las que empiezan por `#` se ignoran.
//! * `!` delante crea una excepción. Gana el **último** patrón que coincide.
//! * `*` no cruza `/`, `?` es un carácter, `**` cualquier número de directorios.
//! * Un patrón que coincide con un directorio excluye todo su contenido.
//! * Las rutas se limpian (`./a/../b` → `b`) y se quita la `/` inicial.

use regex::Regex;

#[derive(Debug, Clone)]
struct Pattern {
    text: String,
    re: Regex,
    exception: bool,
    /// Añadido por la aplicación (Dockerfile y `.dockerignore`), no por el usuario.
    builtin: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IgnoreMatcher {
    patterns: Vec<Pattern>,
}

/// Limpia una ruta como `filepath.Clean` (con `/`).
pub fn clean(p: &str) -> String {
    let abs = p.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if out.last().is_some_and(|l| *l != "..") {
                    out.pop();
                } else if !abs {
                    out.push("..");
                }
            }
            c => out.push(c),
        }
    }
    let s = out.join("/");
    match (abs, s.is_empty()) {
        (true, _) => format!("/{s}"),
        (false, true) => ".".into(),
        (false, false) => s,
    }
}

fn compile(pattern: &str) -> Option<Regex> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut re = String::from("^");
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' => {
                if chars.get(i + 1) == Some(&'*') {
                    i += 1;
                    if chars.get(i + 1) == Some(&'/') {
                        // "**/" → cero o más directorios.
                        i += 1;
                        re.push_str("(.*/)?");
                    } else {
                        re.push_str(".*");
                    }
                } else {
                    re.push_str("[^/]*");
                }
            }
            '?' => re.push_str("[^/]"),
            '\\' => {
                i += 1;
                let n = chars.get(i)?;
                re.push_str(&regex::escape(&n.to_string()));
            }
            '[' => {
                // Clase de caracteres: se copia hasta el "]" de cierre.
                let end = chars[i + 1..].iter().position(|&x| x == ']')? + i + 1;
                let mut class: String = chars[i + 1..end].iter().collect();
                if let Some(rest) = class.strip_prefix('!') {
                    class = format!("^{rest}");
                }
                re.push('[');
                re.push_str(&class.replace('\\', "\\\\"));
                re.push(']');
                i = end;
            }
            c => re.push_str(&regex::escape(&c.to_string())),
        }
        i += 1;
    }
    re.push('$');
    Regex::new(&re).ok()
}

impl IgnoreMatcher {
    /// Lee el contenido de un `.dockerignore`. Devuelve también las líneas que no
    /// se pudieron interpretar (el CLI las rechaza con un error).
    pub fn parse(content: &str) -> (Self, Vec<String>) {
        let mut patterns = Vec::new();
        let mut invalid = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (exception, raw) = match line.strip_prefix('!') {
                Some(r) => (true, r.trim()),
                None => (false, line),
            };
            if raw.is_empty() {
                invalid.push(line.to_owned());
                continue;
            }
            let mut p = clean(raw);
            if p.len() > 1 && p.starts_with('/') {
                p = p[1..].to_owned();
            }
            match compile(&p) {
                Some(re) => patterns.push(Pattern {
                    text: p,
                    re,
                    exception,
                    builtin: false,
                }),
                None => invalid.push(line.to_owned()),
            }
        }
        (Self { patterns }, invalid)
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn has_exceptions(&self) -> bool {
        self.patterns.iter().any(|p| p.exception)
    }

    /// ¿Puede alguna excepción incluir algo dentro de `dir` (excluido)? Si no,
    /// el recorrido no entra en la carpeta (p. ej. `node_modules`).
    pub fn may_include_inside(&self, dir: &str) -> bool {
        let dir = clean(dir);
        let with_slash = format!("{dir}/");
        self.patterns.iter().filter(|p| p.exception).any(|p| {
            // Parte literal del patrón, hasta el primer comodín.
            let lit: String = p
                .text
                .chars()
                .take_while(|c| !matches!(c, '*' | '?' | '[' | '\\'))
                .collect();
            let wild = lit.len() < p.text.len();
            lit.starts_with(&with_slash) || (wild && with_slash.starts_with(&lit))
        })
    }

    /// Añade excepciones al final (el Dockerfile y el propio `.dockerignore`
    /// siempre se envían, como hace el CLI).
    pub fn keep(&mut self, path: &str) {
        let p = clean(path);
        if let Some(re) = compile(&regex_literal_glob(&p)) {
            self.patterns.push(Pattern {
                text: p,
                re,
                exception: true,
                builtin: true,
            });
        }
    }

    /// ¿Queda excluida la ruta (relativa al contexto, separada por `/`)?
    pub fn excluded(&self, path: &str) -> bool {
        let path = clean(path);
        let parents: Vec<&str> = match path.rsplit_once('/') {
            Some((dir, _)) => dir.split('/').collect(),
            None => Vec::new(),
        };
        let mut matched = false;
        for p in &self.patterns {
            let mut m = p.re.is_match(&path);
            if !m {
                for i in 0..parents.len() {
                    if p.re.is_match(&parents[..=i].join("/")) {
                        m = true;
                        break;
                    }
                }
            }
            if m {
                matched = !p.exception;
            }
        }
        matched
    }

    /// Patrones del usuario, para mostrarlos.
    pub fn patterns(&self) -> Vec<String> {
        self.patterns
            .iter()
            .filter(|p| !p.builtin)
            .map(|p| {
                if p.exception {
                    format!("!{}", p.text)
                } else {
                    p.text.clone()
                }
            })
            .collect()
    }
}

/// Escapa los comodines para que una ruta concreta solo coincida consigo misma.
fn regex_literal_glob(p: &str) -> String {
    let mut s = String::new();
    for c in p.chars() {
        if matches!(c, '*' | '?' | '[' | '\\') {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(s: &str) -> IgnoreMatcher {
        let (m, bad) = IgnoreMatcher::parse(s);
        assert!(bad.is_empty(), "{bad:?}");
        m
    }

    #[test]
    fn cleans_paths_like_go() {
        assert_eq!(clean("./a/../b/"), "b");
        assert_eq!(clean("/a//b/./c"), "/a/b/c");
        assert_eq!(clean("../x"), "../x");
        assert_eq!(clean(""), ".");
    }

    #[test]
    fn basic_patterns_and_directories() {
        let i = m("# comentario\n\nnode_modules\n*.log\n/secret.txt\ntmp?\n");
        assert!(i.excluded("node_modules"));
        assert!(
            i.excluded("node_modules/a/b.js"),
            "un directorio excluye su contenido"
        );
        assert!(i.excluded("app.log"));
        assert!(!i.excluded("logs/app.log"), "* no cruza directorios");
        assert!(i.excluded("secret.txt"));
        assert!(i.excluded("tmp1") && !i.excluded("tmp12"));
        assert!(!i.excluded("src/main.rs"));
    }

    #[test]
    fn double_star_and_exceptions_last_match_wins() {
        let i = m("**/*.tmp\n*.md\n!README.md\ndocs\n!docs/keep.txt\n");
        assert!(i.excluded("a.tmp") && i.excluded("x/y/z.tmp"));
        assert!(i.excluded("CHANGELOG.md"));
        assert!(!i.excluded("README.md"));
        assert!(i.excluded("docs/other.txt"));
        assert!(!i.excluded("docs/keep.txt"));
        assert!(i.has_exceptions());
        let i = m("!README.md\n*.md\n");
        assert!(
            i.excluded("README.md"),
            "gana el último patrón que coincide"
        );
    }

    #[test]
    fn classes_escapes_and_regex_metacharacters() {
        let i = m("file[0-9].txt\na+b(c).txt\nweird\\*name\n");
        assert!(i.excluded("file3.txt") && !i.excluded("filex.txt"));
        assert!(i.excluded("a+b(c).txt"));
        assert!(i.excluded("weird*name") && !i.excluded("weirdXname"));
    }

    #[test]
    fn keep_forces_inclusion() {
        let mut i = m("*\n");
        assert!(i.excluded("Dockerfile"));
        i.keep("Dockerfile");
        i.keep(".dockerignore");
        assert!(!i.excluded("Dockerfile") && !i.excluded(".dockerignore"));
        assert!(i.excluded("otro"));
        assert_eq!(
            i.patterns(),
            vec!["*"],
            "las excepciones internas no se muestran"
        );
    }

    #[test]
    fn only_relevant_exceptions_open_excluded_directories() {
        let mut i = m("node_modules\ndocs\n!docs/keep.txt\nbuild\n!**/*.md\n");
        i.keep("Dockerfile");
        i.keep(".dockerignore");
        assert!(i.may_include_inside("docs"));
        assert!(
            i.may_include_inside("node_modules"),
            "!**/*.md puede incluir algo en cualquier carpeta"
        );
        let mut j = m("node_modules\n");
        j.keep("Dockerfile");
        j.keep("docker/Dockerfile.prod");
        assert!(!j.may_include_inside("node_modules"));
        assert!(j.may_include_inside("docker"));
    }

    #[test]
    fn unparseable_lines_are_reported() {
        let (_, bad) = IgnoreMatcher::parse("ok\n!\nmal[\n");
        assert_eq!(bad, vec!["!", "mal["]);
    }
}
