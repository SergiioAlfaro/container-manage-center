// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useState } from "react";
import { api } from "@/shared/ipc/client";
import type { ImageRef } from "@/shared/ipc/bindings/ImageRef";

export type RefCheck = { state: "empty" } | { state: "checking" } | { state: "ok"; ref: ImageRef } | { state: "error"; code: string };

/** Forma corta como la muestra Docker (`nginx:latest`, `registro:5000/app:v1`). */
export function familiar(r: ImageRef): string {
  const name = r.registry === "docker.io" ? r.repository.replace(/^library\//, "") : `${r.registry}/${r.repository}`;
  if (r.digest) return r.tag ? `${name}:${r.tag}@${r.digest}` : `${name}@${r.digest}`;
  return `${name}:${r.tag ?? "latest"}`;
}

/** Comprueba una referencia con las reglas de Docker (en el backend, sin hablar con Docker). */
export function useRefCheck(text: string, delay = 150): RefCheck {
  const [check, setCheck] = useState<RefCheck>({ state: "empty" });
  useEffect(() => {
    const v = text.trim();
    if (!v) {
      setCheck({ state: "empty" });
      return;
    }
    setCheck({ state: "checking" });
    let alive = true;
    const t = window.setTimeout(() => {
      api
        .imageRefParse(v)
        .then((ref) => alive && setCheck({ state: "ok", ref }))
        .catch((e) => alive && setCheck({ state: "error", code: (e as { code: string }).code }));
    }, delay);
    return () => {
      alive = false;
      window.clearTimeout(t);
    };
  }, [text, delay]);
  return check;
}
