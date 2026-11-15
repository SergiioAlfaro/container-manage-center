// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Representación de una lista de argumentos como línea de comando POSIX.
//!
//! Solo se usa para **mostrar** y **copiar** comandos equivalentes. La aplicación
//! nunca ejecuta estas cadenas a través de un shell.

/// Caracteres que pueden aparecer sin comillas en un argumento.
fn is_safe(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '-' | '_' | '.' | '/' | ':' | '=' | '@' | '%' | '+' | ',')
}

/// Entrecomilla un argumento para un shell POSIX.
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && arg.chars().all(is_safe) {
        return arg.to_owned();
    }
    // Comillas simples; una comilla simple interna se cierra, se escapa y se reabre.
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('\'');
    for c in arg.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Une programa y argumentos en una línea que un shell POSIX interpretaría
/// exactamente como esos mismos argumentos.
pub fn render_argv<S: AsRef<str>>(program: &str, args: &[S]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(quote_arg(program));
    parts.extend(args.iter().map(|a| quote_arg(a.as_ref())));
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_arguments_are_not_quoted() {
        assert_eq!(
            render_argv(
                "docker",
                &["run", "-d", "--name", "web", "-p", "8080:80", "nginx:1.27"]
            ),
            "docker run -d --name web -p 8080:80 nginx:1.27"
        );
    }

    #[test]
    fn spaces_and_metacharacters_are_quoted() {
        assert_eq!(quote_arg("hola mundo"), "'hola mundo'");
        assert_eq!(quote_arg("a;rm -rf /"), "'a;rm -rf /'");
        assert_eq!(quote_arg("$(whoami)"), "'$(whoami)'");
        assert_eq!(quote_arg("`id`"), "'`id`'");
        assert_eq!(quote_arg("a|b&c>d"), "'a|b&c>d'");
    }

    #[test]
    fn single_quotes_are_escaped() {
        assert_eq!(quote_arg("it's"), "'it'\\''s'");
    }

    #[test]
    fn empty_argument_is_preserved() {
        assert_eq!(render_argv("docker", &["exec", ""]), "docker exec ''");
    }

    #[test]
    fn non_ascii_is_quoted() {
        assert_eq!(quote_arg("señal"), "'señal'");
    }
}
