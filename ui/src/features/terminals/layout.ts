/**
 * Distribución de los paneles según cuántas sesiones se ven (D-064):
 * 1 ocupa todo; 2 en dos columnas; 3 con una columna a la izquierda y dos
 * filas a la derecha; 4 en cuadrícula 2×2; más de 4, filas de dos con
 * desplazamiento vertical (todas siguen vivas y visibles al desplazarse).
 */
export type Layout = "empty" | "single" | "columns" | "three" | "grid" | "scroll";

export function layoutFor(count: number): Layout {
  if (count <= 0) return "empty";
  if (count === 1) return "single";
  if (count === 2) return "columns";
  if (count === 3) return "three";
  if (count === 4) return "grid";
  return "scroll";
}

/** Agrupa en filas de dos (la última puede quedar con uno). */
export function pairs<T>(items: T[]): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < items.length; i += 2) out.push(items.slice(i, i + 2));
  return out;
}
