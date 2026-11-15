// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import es from "./i18n/locales/es.json";
import { platformKey } from "./platform";

const has = (k: string) => k.split(".").reduce<unknown>((o, p) => (o as Record<string, unknown>)?.[p], es) !== undefined;

describe("plataforma (1.9.0)", () => {
  it("en Windows se usa la variante …Win si existe; si no, la general", () => {
    expect(platformKey("backup.jobsHint", has, true)).toBe("backup.jobsHintWin");
    expect(platformKey("backup.noJobs", has, true)).toBe("backup.noJobs");
    expect(platformKey("backup.jobsHint", has, false)).toBe("backup.jobsHint");
  });

  it("cada texto con variante de Windows la tiene", () => {
    for (const k of [
      "backup.noSystemd",
      "backup.jobsHint",
      "backup.timer.foreign",
      "backup.timer.problem",
      "backup.timer.removeBody",
      "backup.timer.authBody",
      "backup.timer.authCheck",
      "components.unsupported",
    ]) {
      expect(has(`${k}Win`), k).toBe(true);
    }
  });
});
