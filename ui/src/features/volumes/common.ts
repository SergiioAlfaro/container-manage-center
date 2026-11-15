// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useQuery } from "@tanstack/react-query";
import { useApi } from "@/app/server";
import type { VolumeSummary } from "@/shared/ipc/bindings/VolumeSummary";
import { keys } from "@/shared/query";

/** Nombre corto para mostrar: los anónimos tienen 64 caracteres. */
export function volumeLabel(v: Pick<VolumeSummary, "name" | "anonymous">): string {
  return v.anonymous ? v.name.slice(0, 12) : v.name;
}

/** Hook compartido: tamaños bajo demanda (`docker system df --verbose`). */
export function useVolumeSizes() {
  const api = useApi();
  return useQuery({ queryKey: keys.volumeSizes, queryFn: api.volumeSizes, enabled: false, staleTime: Infinity });
}
