import { useQuery } from "@tanstack/react-query";
import { api } from "@/shared/ipc/client";
import type { VolumeSummary } from "@/shared/ipc/bindings/VolumeSummary";
import { keys } from "@/shared/query";

/** Nombre corto para mostrar: los anónimos tienen 64 caracteres. */
export function volumeLabel(v: Pick<VolumeSummary, "name" | "anonymous">): string {
  return v.anonymous ? v.name.slice(0, 12) : v.name;
}

/** Hook compartido: tamaños bajo demanda (`docker system df --verbose`). */
export function useVolumeSizes() {
  return useQuery({ queryKey: keys.volumeSizes, queryFn: api.volumeSizes, enabled: false, staleTime: Infinity });
}
