/**
 * Doble de xterm.js para jsdom (no tiene canvas). Registra lo que se escribe y
 * permite simular lo que teclea el usuario.
 */
type Fn<T> = (v: T) => void;

export class FakeTerminal {
  static last: FakeTerminal | null = null;
  /** Todas las creadas (el apartado «Terminales» tiene varias a la vez). */
  static all: FakeTerminal[] = [];
  disposed = false;
  opened = false;
  focused = 0;
  cols = 80;
  rows = 24;
  written: Array<string | Uint8Array> = [];
  private data: Array<Fn<string>> = [];
  private resize: Array<Fn<{ cols: number; rows: number }>> = [];
  constructor(public options: unknown) {
    FakeTerminal.last = this;
    FakeTerminal.all.push(this);
  }
  loadAddon() {}
  open() {
    this.opened = true;
  }
  focus() {
    this.focused++;
  }
  reset() {
    this.written = [];
  }
  dispose() {
    this.disposed = true;
  }
  write(d: string | Uint8Array) {
    this.written.push(d);
  }
  onData(f: Fn<string>) {
    this.data.push(f);
    return { dispose: () => (this.data = this.data.filter((x) => x !== f)) };
  }
  onResize(f: Fn<{ cols: number; rows: number }>) {
    this.resize.push(f);
    return { dispose: () => (this.resize = this.resize.filter((x) => x !== f)) };
  }
  /** Simula pulsaciones del usuario. */
  type(s: string) {
    this.data.forEach((f) => f(s));
  }
  doResize(cols: number, rows: number) {
    this.cols = cols;
    this.rows = rows;
    this.resize.forEach((f) => f({ cols, rows }));
  }
  /** Texto escrito, decodificado. */
  text(): string {
    return this.written.map((w) => (typeof w === "string" ? w : new TextDecoder().decode(w))).join("");
  }
}

export class FakeFit {
  fit() {}
}
