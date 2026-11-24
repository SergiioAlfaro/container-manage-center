// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import { dockerEvent } from "@/test/apiMock";
import { eventResource, formatEventTime, fromLocalInput, invalidationsFor, matchesLocal, toLocalInput } from "./model";

describe("eventos: modelo", () => {
  it("cada evento recarga solo lo que puede haber cambiado", () => {
    const k = (kind: string, action: string) => invalidationsFor({ kind, action }).map((x) => JSON.stringify(x));
    expect(k("container", "die")).toEqual(expect.arrayContaining(['["containers"]', '["overview"]', '["composeProjects"]', '["binds"]', '["container"]']));
    expect(k("container", "health_status: healthy")).toContain('["containers"]');
    // Comprobaciones de salud y terminales no recargan nada.
    expect(k("container", "exec_create: sh -c true")).toEqual([]);
    expect(k("container", "exec_die")).toEqual([]);
    expect(k("image", "pull")).toEqual(['["images"]', '["image"]', '["overview"]']);
    expect(k("volume", "destroy")).toEqual(['["volumes"]', '["volumesBrief"]', '["volume"]', '["volumeSizes"]', '["overview"]']);
    expect(k("network", "connect")).toEqual(expect.arrayContaining(['["networks"]', '["networksBrief"]', '["containers"]']));
    expect(k("daemon", "reload")).toEqual(['["overview"]']);
    expect(k("plugin", "enable")).toEqual([]);
  });

  it("nombres, horas y filtros", () => {
    expect(eventResource(dockerEvent())).toBe("web");
    expect(eventResource(dockerEvent({ name: null, kind: "image", actorId: `sha256:${"a".repeat(64)}` }))).toBe("aaaaaaaaaaaa");
    expect(eventResource(dockerEvent({ name: null, kind: "volume", actorId: "datos" }))).toBe("datos");
    const e = dockerEvent({ timeNano: new Date(2026, 10, 7, 9, 5, 3, 42).getTime() * 1e6 });
    expect(formatEventTime(e, "es")).toBe("09:05:03.042");
    expect(formatEventTime(e, "es", true)).toBe("07/11/2026 09:05:03.042");
    const secs = Math.floor(new Date(2026, 10, 7, 9, 5).getTime() / 1000);
    expect(toLocalInput(secs)).toBe("2026-11-07T09:05");
    expect(fromLocalInput("2026-11-07T09:05")).toBe(secs);
    expect(fromLocalInput("")).toBeNull();
    expect(matchesLocal(e, { types: ["image"], text: "" })).toBe(false);
    expect(matchesLocal(e, { types: ["container"], text: "NGINX" })).toBe(true);
    expect(matchesLocal(e, { types: [], text: "redis" })).toBe(false);
  });
});
