// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Gráfico de líneas en SVG propio (E5, D-106): sin biblioteca. Las líneas van
 * en SVG y los textos en HTML (no se deforman). Los huecos (`null`) cortan la
 * línea; las desconexiones se sombrean. Cada serie tiene color y trazo
 * propios, para no depender solo del color.
 */
import { useId, useMemo, useState } from "react";

export interface ChartSeries {
  name: string;
  /** [ms, valor | null] en orden. */
  points: Array<[number, number | null]>;
}

export const CHART_COLORS = ["var(--beacon)", "var(--signal-run)", "var(--signal-warn)", "var(--signal-ok)"];
export const CHART_DASHES = ["", "6 3", "2 3", "8 3 2 3"];

const W = 600;

function path(points: Array<[number, number | null]>, x: (t: number) => number, y: (v: number) => number): string {
  let d = "";
  // Puntos del tramo actual: un punto suelto (entre dos huecos) se dibuja como
  // un punto, que si no no se vería.
  let run = 0;
  const end = () => {
    if (run === 1) d += "l0.01,0 ";
    run = 0;
  };
  for (const [t, v] of points) {
    if (v === null || !Number.isFinite(v)) {
      end();
      continue;
    }
    d += `${run > 0 ? "L" : "M"}${x(t).toFixed(1)},${y(v).toFixed(1)} `;
    run++;
  }
  end();
  return d.trim();
}

/** Tramos sin conexión: [inicio, fin] en ms. */
export type Gap = [number, number];

export function LineChart({
  label,
  series,
  from,
  to,
  format,
  max,
  gaps = [],
  height = 140,
  timeFormat,
  emptyText,
}: {
  label: string;
  series: ChartSeries[];
  from: number;
  to: number;
  format: (v: number) => string;
  /** Máximo fijo del eje (p. ej. 100 en porcentajes de memoria). */
  max?: number;
  gaps?: Gap[];
  height?: number;
  timeFormat: (t: number) => string;
  emptyText: string;
}) {
  const id = useId();
  const [hover, setHover] = useState<number | null>(null);
  const values = series.flatMap((s) => s.points.map((p) => p[1]).filter((v): v is number => v !== null && Number.isFinite(v)));
  const top = useMemo(() => {
    const m = Math.max(0, ...values);
    if (max !== undefined) return Math.max(max, m);
    return m > 0 ? m * 1.15 : 1;
  }, [values, max]);
  const span = Math.max(to - from, 1);
  const x = (t: number) => ((t - from) / span) * W;
  const y = (v: number) => height - 4 - (v / top) * (height - 8);
  const last = series.map((s) => [...s.points].reverse().find((p) => p[1] !== null)?.[1] ?? null);
  const summary = `${label}: ${series.map((s, i) => `${s.name} ${last[i] === null ? "—" : format(last[i]!)}`).join(", ")}`;
  // Valores en el punto bajo el ratón.
  const at = hover === null ? null : from + (hover / W) * span;
  const near = at === null ? null : series.map((s) => s.points.reduce<[number, number | null] | null>((b, p) => (b === null || Math.abs(p[0] - at) < Math.abs(b[0] - at) ? p : b), null));
  return (
    <figure className="rounded-xl border border-line bg-surface px-4 py-3">
      <figcaption className="mb-1 flex items-baseline justify-between gap-2 text-[12.5px]">
        <span className="font-medium">{label}</span>
        {values.length > 0 && <span className="text-[11.5px] text-ink-3 tabular-nums">{format(top)}</span>}
      </figcaption>
      {values.length === 0 ? (
        <p className="flex items-center justify-center text-[12px] text-ink-3" style={{ height }}>
          {emptyText}
        </p>
      ) : (
        <div className="relative">
          <svg
            viewBox={`0 0 ${W} ${height}`}
            preserveAspectRatio="none"
            role="img"
            aria-label={summary}
            className="w-full"
            style={{ height }}
            onMouseMove={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              setHover(((e.clientX - r.left) / Math.max(r.width, 1)) * W);
            }}
            onMouseLeave={() => setHover(null)}
          >
            <defs>
              <pattern id={`${id}-gap`} width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
                <line x1="0" y1="0" x2="0" y2="6" stroke="var(--line-strong)" strokeWidth="2" />
              </pattern>
            </defs>
            {gaps.map(([a, b]) => (
              <rect key={a} x={x(a)} y={0} width={Math.max(x(b) - x(a), 2)} height={height} fill={`url(#${id}-gap)`} opacity="0.6" />
            ))}
            {[0.5, 1].map((f) => (
              <line key={f} x1="0" x2={W} y1={y(top * f)} y2={y(top * f)} stroke="var(--log-rule)" vectorEffect="non-scaling-stroke" />
            ))}
            <line x1="0" x2={W} y1={height - 4} y2={height - 4} stroke="var(--line-strong)" vectorEffect="non-scaling-stroke" />
            {series.map((s, i) => (
              <path
                key={s.name}
                d={path(s.points, x, y)}
                fill="none"
                stroke={CHART_COLORS[i % CHART_COLORS.length]}
                strokeDasharray={CHART_DASHES[i % CHART_DASHES.length]}
                strokeWidth="1.8"
                strokeLinejoin="round"
                strokeLinecap="round"
                vectorEffect="non-scaling-stroke"
              />
            ))}
            {hover !== null && <line x1={hover} x2={hover} y1="0" y2={height} stroke="var(--ink-3)" strokeDasharray="2 2" vectorEffect="non-scaling-stroke" />}
          </svg>
          {near && at !== null && (
            <div className="pointer-events-none absolute right-1 top-1 rounded-md border border-line bg-surface/95 px-2 py-1 text-[11.5px] shadow-sm tabular-nums">
              <div className="text-ink-3">{timeFormat(at)}</div>
              {series.map((s, i) => (
                <div key={s.name}>
                  {s.name}: {near[i]?.[1] === null || near[i] === null ? "—" : format(near[i]![1]!)}
                </div>
              ))}
            </div>
          )}
          <div className="mt-1 flex justify-between text-[11px] text-ink-3 tabular-nums" aria-hidden>
            <span>{timeFormat(from)}</span>
            <span>{timeFormat(to)}</span>
          </div>
        </div>
      )}
      {series.length > 0 && (
        <ul className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1 text-[12px]" aria-label={label}>
          {series.map((s, i) => (
            <li key={s.name} className="flex items-center gap-1.5">
              <svg width="22" height="8" aria-hidden>
                <line x1="0" x2="22" y1="4" y2="4" stroke={CHART_COLORS[i % CHART_COLORS.length]} strokeDasharray={CHART_DASHES[i % CHART_DASHES.length]} strokeWidth="2" />
              </svg>
              <span className="truncate">{s.name}</span>
              <span className="text-ink-3 tabular-nums">{last[i] === null ? "—" : format(last[i]!)}</span>
            </li>
          ))}
        </ul>
      )}
    </figure>
  );
}
