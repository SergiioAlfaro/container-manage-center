import { Component, type ErrorInfo, type ReactNode } from "react";
import i18n from "@/shared/i18n";

interface State {
  error: Error | null;
}

/**
 * Evita la pantalla en blanco ante un error de la interfaz: muestra el error
 * (para poder notificarlo) y permite recargar la ventana.
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Error de la interfaz", error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    const t = i18n.t.bind(i18n);
    return (
      <div role="alert" className="mx-auto mt-16 max-w-[640px] px-8">
        <h1 className="text-[17px] font-semibold">{t("crash.title")}</h1>
        <p className="mt-2 text-[13.5px] text-ink-2">{t("crash.body")}</p>
        <pre className="selectable mt-4 max-h-80 overflow-auto rounded-lg bg-code p-3 font-mono text-[12px] whitespace-pre-wrap [overflow-wrap:anywhere]">
          {error.name}: {error.message}
          {"\n"}
          {error.stack}
        </pre>
        <button type="button" onClick={() => window.location.reload()} className="mt-4 h-8 rounded-md bg-beacon px-3 text-[13px] font-medium text-beacon-ink hover:bg-beacon-strong">
          {t("crash.reload")}
        </button>
      </div>
    );
  }
}
