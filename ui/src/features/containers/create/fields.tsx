/** Piezas de formulario del asistente de creación. */
import { useId, type InputHTMLAttributes, type ReactNode } from "react";
import { useServerView } from "@/app/server";
import { AlertTriangle, CircleAlert, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";
import { Button, cx } from "@/shared/ui/controls";

export const inputClass =
  "h-8 w-full min-w-0 rounded-md border bg-surface px-2.5 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon disabled:opacity-60";

/** Texto traducido de un problema de validación. */
export function useIssueText() {
  const { t } = useTranslation();
  // En un servidor remoto, las rutas son del servidor (variantes «_remote»).
  const context = useServerView()?.remote ? "remote" : undefined;
  return (i: SpecIssue) => t(`issues.${i.code}`, { ...i.params, context, defaultValue: i.code });
}

export function IssueList({ issues }: { issues: SpecIssue[] }) {
  const text = useIssueText();
  if (issues.length === 0) return null;
  return (
    <ul className="mt-1 space-y-0.5">
      {issues.map((i, n) => (
        <li key={`${i.code}-${n}`} className={cx("flex items-start gap-1 text-[12px]", i.severity === "error" ? "text-err" : "text-warn")}>
          {i.severity === "error" ? <CircleAlert size={13} className="mt-px shrink-0" /> : <AlertTriangle size={13} className="mt-px shrink-0" />}
          <span>{text(i)}</span>
        </li>
      ))}
    </ul>
  );
}

export function Field({
  label,
  hint,
  issues = [],
  children,
  id,
}: {
  label: string;
  hint?: ReactNode;
  issues?: SpecIssue[];
  children: (id: string, invalid: boolean) => ReactNode;
  id?: string;
}) {
  const auto = useId();
  const fid = id ?? auto;
  const invalid = issues.some((i) => i.severity === "error");
  return (
    <div className="min-w-0">
      <label htmlFor={fid} className="mb-1 block text-[12.5px] font-medium text-ink-2">
        {label}
      </label>
      {children(fid, invalid)}
      {hint && issues.length === 0 && <p className="mt-1 text-[12px] text-ink-3">{hint}</p>}
      <IssueList issues={issues} />
    </div>
  );
}

export function TextInput({ invalid, className, ...rest }: InputHTMLAttributes<HTMLInputElement> & { invalid?: boolean }) {
  return <input spellCheck={false} autoComplete="off" className={cx(inputClass, invalid ? "border-err" : "border-line", "font-mono text-[12.5px]", className)} {...rest} />;
}

export function Select({ invalid, className, children, ...rest }: React.SelectHTMLAttributes<HTMLSelectElement> & { invalid?: boolean }) {
  return (
    <select className={cx(inputClass, invalid ? "border-err" : "border-line", "pr-1", className)} {...rest}>
      {children}
    </select>
  );
}

/** Sección con título dentro de una pestaña. */
export function Group({ title, description, children, action }: { title: string; description?: string; children: ReactNode; action?: ReactNode }) {
  return (
    <section className="space-y-3">
      <div className="flex items-end gap-3">
        <div className="min-w-0 flex-1">
          <h3 className="text-[13.5px] font-semibold">{title}</h3>
          {description && <p className="text-[12.5px] text-ink-3">{description}</p>}
        </div>
        {action}
      </div>
      {children}
    </section>
  );
}

/** Lista editable de filas (variables, puertos, montajes…). */
export function RowList<T>({
  rows,
  onChange,
  make,
  render,
  addLabel,
  removeLabel,
  empty,
}: {
  rows: T[];
  onChange: (rows: T[]) => void;
  make: () => T;
  render: (row: T, set: (r: T) => void, index: number) => ReactNode;
  addLabel: string;
  removeLabel: (index: number) => string;
  empty: string;
}) {
  return (
    <div className="space-y-2">
      {rows.length === 0 && <p className="rounded-md border border-dashed border-line px-3 py-2.5 text-[12.5px] text-ink-3">{empty}</p>}
      {rows.map((r, i) => (
        <div key={i} className="flex items-start gap-2">
          <div className="min-w-0 flex-1">{render(r, (nr) => onChange(rows.map((x, j) => (j === i ? nr : x))), i)}</div>
          <button
            type="button"
            onClick={() => onChange(rows.filter((_, j) => j !== i))}
            aria-label={removeLabel(i)}
            title={removeLabel(i)}
            className="mt-0.5 grid h-7 w-7 shrink-0 place-items-center rounded-md text-ink-3 hover:bg-err/10 hover:text-err"
          >
            <X size={15} />
          </button>
        </div>
      ))}
      <Button size="sm" onClick={() => onChange([...rows, make()])}>
        <Plus size={14} />
        {addLabel}
      </Button>
    </div>
  );
}
