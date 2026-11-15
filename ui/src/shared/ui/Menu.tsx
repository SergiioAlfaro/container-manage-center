// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Menú de acciones accesible (0.9.2): botón con `aria-haspopup="menu"`, lista
 * con `role="menu"`, flechas para moverse, Escape para cerrar y clic fuera.
 * Se puede abrir desde fuera (`open`), p. ej. con el botón derecho en una fila.
 *
 * La lista se pinta en `document.body` con posición fija junto al botón: dentro
 * de una tabla con desplazamiento, un menú absoluto quedaría recortado (lo
 * mostró la aplicación real). Se abre hacia arriba si no cabe debajo y se
 * cierra al desplazar o redimensionar.
 */
import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cx } from "./controls";

export interface MenuItem {
  id: string;
  label: string;
  icon?: ReactNode;
  onSelect: () => void;
  disabled?: boolean;
  /** Descripción emergente (p. ej. por qué está desactivada). */
  title?: string;
  danger?: boolean;
}

export function Menu({
  label,
  trigger,
  items,
  open,
  onOpenChange,
  align = "right",
  buttonClassName,
}: {
  /** Nombre accesible del botón. */
  label: string;
  trigger: ReactNode;
  items: MenuItem[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  align?: "left" | "right";
  /** Clases del botón (por defecto, un botón cuadrado de icono). */
  buttonClassName?: string;
}) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const [pos, setPos] = useState<CSSProperties>({ visibility: "hidden" });

  const entries = () => Array.from(list.current?.querySelectorAll<HTMLButtonElement>("[role=menuitem]:not([disabled])") ?? []);

  // Posición junto al botón, calculada antes de pintar.
  useLayoutEffect(() => {
    if (!open) return;
    const b = button.current?.getBoundingClientRect();
    const h = list.current?.offsetHeight ?? 0;
    if (!b) return;
    const below = b.bottom + 4 + h <= window.innerHeight - 8;
    const top = below ? b.bottom + 4 : Math.max(8, b.top - 4 - h);
    setPos(align === "right" ? { top, right: Math.max(8, window.innerWidth - b.right) } : { top, left: Math.max(8, b.left) });
  }, [open, align]);

  useEffect(() => {
    if (!open) return;
    entries()[0]?.focus();
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!root.current?.contains(t) && !list.current?.contains(t)) onOpenChange(false);
    };
    // Al desplazar o redimensionar, el menú se quedaría separado de su fila.
    const onMove = (e: Event) => {
      if (!list.current?.contains(e.target as Node)) onOpenChange(false);
    };
    document.addEventListener("mousedown", onDown);
    window.addEventListener("scroll", onMove, true);
    window.addEventListener("resize", onMove);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("scroll", onMove, true);
      window.removeEventListener("resize", onMove);
      setPos({ visibility: "hidden" });
    };
  }, [open, onOpenChange]);

  const close = (refocus = true) => {
    onOpenChange(false);
    if (refocus) button.current?.focus();
  };

  return (
    <div ref={root} className="relative inline-block">
      <button
        ref={button}
        type="button"
        aria-label={label}
        // Sin descripción emergente con el menú abierto: taparía sus opciones.
        title={open ? undefined : label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => onOpenChange(!open)}
        className={buttonClassName ?? cx("grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk hover:text-ink", open && "bg-surface-sunk text-ink")}
      >
        {trigger}
      </button>
      {open &&
        createPortal(
        <ul
          ref={list}
          id={id}
          role="menu"
          aria-label={label}
          onKeyDown={(e) => {
            const all = entries();
            const i = all.indexOf(document.activeElement as HTMLButtonElement);
            if (e.key === "ArrowDown") {
              e.preventDefault();
              all[(i + 1) % all.length]?.focus();
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              all[(i - 1 + all.length) % all.length]?.focus();
            } else if (e.key === "Home") {
              e.preventDefault();
              all[0]?.focus();
            } else if (e.key === "End") {
              e.preventDefault();
              all[all.length - 1]?.focus();
            } else if (e.key === "Escape") {
              e.preventDefault();
              e.stopPropagation();
              close();
            } else if (e.key === "Tab") {
              close(false);
            }
          }}
          style={{ position: "fixed", ...pos }}
          className="panel-enter z-50 min-w-[250px] rounded-lg border border-line bg-surface py-1 text-left shadow-xl"
        >
          {items.map((it) => (
            <li key={it.id} role="none">
              <button
                type="button"
                role="menuitem"
                disabled={it.disabled}
                title={it.title}
                onClick={() => {
                  close();
                  it.onSelect();
                }}
                className={cx(
                  "flex w-full items-center gap-2 px-3 py-1.5 text-[13px] focus:bg-surface-2 focus:outline-none enabled:hover:bg-surface-2 disabled:opacity-50",
                  it.danger ? "text-err" : "text-ink",
                )}
              >
                {it.icon && <span className="shrink-0 text-ink-3">{it.icon}</span>}
                {it.label}
              </button>
            </li>
          ))}
        </ul>,
          document.body,
        )}
    </div>
  );
}
