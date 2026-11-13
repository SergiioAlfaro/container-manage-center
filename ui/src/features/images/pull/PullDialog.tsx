import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Checkbox, Dialog } from "@/shared/ui/feedback";
import { Field, TextInput } from "@/features/containers/create/fields";
import { familiar, useRefCheck } from "../useRefCheck";
import { usePulls } from "./pulls";
import { useServerId, useServerView } from "@/app/server";

type AuthMode = "dockerConfig" | "anonymous" | "manual";

export function PullDialog({ initial = "", onClose, onStarted }: { initial?: string; onClose: () => void; onStarted?: () => void }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const start = usePulls((s) => s.start);
  const server = useServerId();
  const [text, setText] = useState(initial);
  const [mode, setMode] = useState<AuthMode>("dockerConfig");
  const [user, setUser] = useState("");
  const [password, setPassword] = useState("");
  // E2 (DCC-SEC-07, D-090): en un servidor remoto, las credenciales de este
  // equipo solo se envían si se marca expresamente (por omisión, anónima).
  const view = useServerView();
  const remote = !!view?.remote;
  const [share, setShare] = useState(false);
  const check = useRefCheck(text);
  const manualOk = mode !== "manual" || (user.trim() !== "" && password !== "");
  const ok = check.state === "ok" && manualOk;

  const submit = () => {
    if (check.state !== "ok" || !manualOk) return;
    const auth = mode === "manual" ? { mode, username: user.trim(), password } : { mode };
    void start(
      server,
      text.trim(),
      auth,
      () => {
        void qc.invalidateQueries({ queryKey: keys.images });
        void qc.invalidateQueries({ queryKey: keys.overview });
      },
      remote && mode === "dockerConfig" && share,
    );
    // La contraseña no se conserva en el formulario.
    setPassword("");
    onStarted?.();
    onClose();
  };

  return (
    <Dialog
      title={t("pull.title")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="primary" disabled={!ok} onClick={submit}>
            <Download size={14} />
            {t("pull.start")}
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        className="space-y-4"
      >
        <Field
          label={t("pull.reference")}
          hint={check.state === "ok" ? t("pull.resolved", { name: `${check.ref.registry}/${check.ref.repository}`, tag: check.ref.digest ?? check.ref.tag ?? "latest" }) : t("pull.referenceHint")}
          issues={check.state === "error" ? [{ severity: "error", field: "reference", code: check.code, params: {} }] : []}
        >
          {(id, inv) => <TextInput id={id} invalid={inv} value={text} onChange={(e) => setText(e.target.value)} placeholder="nginx:1.27" autoFocus />}
        </Field>
        <div className="space-y-2">
          <p className="text-[12.5px] font-medium text-ink-2">{t("pull.auth")}</p>
          <Segmented<AuthMode>
            label={t("pull.auth")}
            value={mode}
            onChange={setMode}
            options={[
              { value: "dockerConfig", label: t("pull.authDocker") },
              { value: "anonymous", label: t("pull.authAnonymous") },
              { value: "manual", label: t("pull.authManual") },
            ]}
          />
          <p className="text-[12px] text-ink-3">{t(`pull.authHint.${mode}`)}</p>
          {remote && mode === "dockerConfig" && (
            <div className="space-y-1">
              <Checkbox checked={share} onChange={setShare} label={t("pull.shareLocal", { server: view?.name ?? "" })} />
              <p className="text-[12px] text-ink-3">{t(share ? "pull.shareLocalOn" : "pull.shareLocalOff")}</p>
            </div>
          )}
          {mode === "manual" && (
            <div className="grid gap-3 sm:grid-cols-2">
              <Field label={t("pull.username")}>{(id) => <TextInput id={id} value={user} onChange={(e) => setUser(e.target.value)} autoComplete="off" />}</Field>
              <Field label={t("pull.password")}>{(id) => <TextInput id={id} type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="new-password" />}</Field>
            </div>
          )}
        </div>
        {check.state === "ok" && (
          <p className="text-[12px] text-ink-3">
            {t("pull.equivalent")} <code className="selectable font-mono text-ink-2">docker pull {familiar(check.ref)}</code>
          </p>
        )}
      </form>
    </Dialog>
  );
}
