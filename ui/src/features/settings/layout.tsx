import type { ReactNode } from "react";

/** Bloques de Configuración. */
export function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mt-8 first:mt-0">
      <h2 className="mb-2 text-[13px] font-semibold text-ink-2">{title}</h2>
      <div className="divide-y divide-line rounded-xl border border-line bg-surface">{children}</div>
    </section>
  );
}

export function Row({
  label,
  hint,
  children,
  htmlFor,
  stacked = false,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  htmlFor?: string;
  /** Valor debajo de la etiqueta, para textos largos como rutas. */
  stacked?: boolean;
}) {
  if (stacked) {
    return (
      <div className="px-5 py-3.5">
        <div className="text-[13.5px] text-ink">{label}</div>
        <div className="mt-1 min-w-0 break-all">{children}</div>
      </div>
    );
  }
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-3.5">
      <div className="min-w-0">
        <label htmlFor={htmlFor} className="text-[13.5px] text-ink">
          {label}
        </label>
        {hint && <p className="mt-0.5 text-[12px] text-ink-3">{hint}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
