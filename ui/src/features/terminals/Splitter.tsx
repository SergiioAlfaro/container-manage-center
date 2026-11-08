import { useRef, useState, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { cx } from "@/shared/ui/controls";
import { SPLIT_MAX, SPLIT_MIN } from "./sessions";

const STEP = 0.05;

/**
 * Separador redimensionable entre dos paneles. Se arrastra con el ratón y,
 * con el foco, se mueve con las flechas (Inicio/Fin para los extremos); doble
 * clic vuelve a repartir a partes iguales. `vertical` separa columnas.
 */
export function Splitter({
  orientation,
  value,
  onChange,
  label,
}: {
  orientation: "vertical" | "horizontal";
  value: number;
  onChange: (v: number) => void;
  label: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);
  const vertical = orientation === "vertical";

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const el = ref.current;
    const parent = el?.parentElement;
    if (!el || !parent) return;
    e.preventDefault();
    el.focus();
    try {
      el.setPointerCapture(e.pointerId);
    } catch {
      /* sin captura (pruebas) */
    }
    setDragging(true);
    const move = (ev: PointerEvent) => {
      const r = parent.getBoundingClientRect();
      const size = vertical ? r.width : r.height;
      if (size <= 0) return;
      onChange(vertical ? (ev.clientX - r.left) / size : (ev.clientY - r.top) / size);
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      setDragging(false);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const dec = vertical ? "ArrowLeft" : "ArrowUp";
    const inc = vertical ? "ArrowRight" : "ArrowDown";
    let v: number | null = null;
    if (e.key === dec) v = value - STEP;
    else if (e.key === inc) v = value + STEP;
    else if (e.key === "Home") v = SPLIT_MIN;
    else if (e.key === "End") v = SPLIT_MAX;
    if (v === null) return;
    e.preventDefault();
    onChange(v);
  };

  return (
    <div
      ref={ref}
      role="separator"
      tabIndex={0}
      aria-label={label}
      aria-orientation={orientation}
      aria-valuemin={Math.round(SPLIT_MIN * 100)}
      aria-valuemax={Math.round(SPLIT_MAX * 100)}
      aria-valuenow={Math.round(value * 100)}
      title={label}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      onDoubleClick={() => onChange(0.5)}
      className={cx(
        "group relative shrink-0 touch-none select-none outline-none",
        vertical ? "w-2 cursor-col-resize" : "h-2 cursor-row-resize",
      )}
    >
      <span
        aria-hidden
        className={cx(
          "absolute rounded-full bg-line transition-colors duration-150 group-hover:bg-beacon/60 group-focus-visible:bg-beacon",
          vertical ? "inset-y-1 left-[3px] w-0.5" : "inset-x-1 top-[3px] h-0.5",
          dragging && "bg-beacon",
        )}
      />
    </div>
  );
}
