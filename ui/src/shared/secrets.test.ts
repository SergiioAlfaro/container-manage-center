// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import { hasCredentials, isSecretEntry, isSecretName } from "./secrets";

// Mismas tablas que `crates/dcc-activity/src/redact.rs` (valores ficticios).
describe("Secretos (E1, DCC-SEC-05)", () => {
  it("nombres secretos exigidos", () => {
    for (const n of ["MYSQL_PWD", "DB_PASS", "PGPASS", "REDIS_AUTH", "SMTP_PASS", "MYSQL_ROOT_PASSWORD", "GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY", "SSH_PRIVATE_KEY", "API_KEY", "SENTRY_DSN", "db.password", "jwt_secret"]) {
      expect(isSecretName(n), n).toBe(true);
    }
  });
  it("nombres corrientes no lo son", () => {
    for (const n of ["PATH", "HOME", "PWD", "OLDPWD", "TZ", "DATABASE_HOST", "POSTGRES_USER", "AUTHOR", "PORT", "MYSQL_PASSWORD_FILE", "VERSION"]) {
      expect(isSecretName(n), n).toBe(false);
    }
  });
  it("valores con credenciales, sea cual sea el nombre", () => {
    expect(isSecretEntry("DATABASE_URL", "postgres://app:ficticio@db/app")).toBe(true);
    expect(isSecretEntry("MYSQL_URL", "app:ficticio@tcp(db:3306)/app")).toBe(true);
    expect(isSecretEntry("REDIS_URL", "redis://db:6379/0")).toBe(false);
    expect(isSecretEntry("SITE_URL", "https://example.org/a:b")).toBe(false);
    expect(hasCredentials("git@github.com:org/repo")).toBe(false);
  });
  it("valores triviales visibles", () => {
    expect(isSecretEntry("AUTH_ENABLED", "true")).toBe(false);
    expect(isSecretEntry("REDIS_AUTH", "")).toBe(false);
    expect(isSecretEntry("REDIS_AUTH", "ficticio")).toBe(true);
  });
});
