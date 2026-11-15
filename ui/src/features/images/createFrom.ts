// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useApp } from "@/app/store";
import { emptySpec } from "@/features/containers/specModel";
import { useContainersNav } from "@/features/containers/store";

/** Abre el asistente de contenedores con la imagen ya elegida. */
export function createContainerFrom(image: string) {
  useContainersNav.getState().showCreate({ spec: { ...emptySpec(), image }, notes: [], sourceName: null });
  useApp.getState().navigate("containers");
}
