import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { CheckCircle2, ShieldAlert, XCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { explain } from "@/features/engine/diagnosis";
import { SourceLabel } from "@/features/engine/SourceLabel";
import { useEngine } from "@/features/engine/store";
import { api } from "@/shared/ipc/client";
import type { ConnectionTest } from "@/shared/ipc/bindings/ConnectionTest";
import type { DockerMode } from "@/shared/ipc/bindings/DockerMode";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";

export function DockerSettings() {
  const { t } = useTranslation();
  const docker = useApp((s) => s.config.docker);
  const updateConfig = useApp((s) => s.updateConfig);
  const { state, reconnect, reconnecting } = useEngine();
  const [mode, setMode] = useState<DockerMode>(docker.mode);
  const [path, setPath] = useState(docker.customSocket);
  const [test, setTest] = useState<ConnectionTest | null>(null);
  useEffect(() => {
    setMode(docker.mode);
    setPath(docker.customSocket);
  }, [docker.mode, docker.customSocket]);

  const draft = { mode, customSocket: path.trim() };
  const dirty = draft.mode !== docker.mode || (draft.mode === "custom" && draft.customSocket !== docker.customSocket);
  const missingPath = mode === "custom" && draft.customSocket === "";

  const tester = useMutation({ mutationFn: () => api.engineTest(draft), onSuccess: setTest, onMutate: () => setTest(null) });
  const compose = useQuery({ queryKey: keys.compose, queryFn: api.composeStatus, enabled: false });

  const testReason = (r: ConnectionTest) => (r.result === "failed" ? t(`diag.problems.${explain(r.diagnosis).problem}.body`, explain(r.diagnosis).params) : "");

  return (
    <section className="mt-8">
      <h2 className="mb-2 text-[13px] font-semibold text-ink-2">{t("settingsDocker.title")}</h2>
      <div className="divide-y divide-line rounded-xl border border-line bg-surface">
        <div className="px-5 py-4">
          <div className="flex items-center justify-between gap-6">
            <span className="text-[13.5px]">{t("settingsDocker.mode")}</span>
            <Segmented<DockerMode>
              label={t("settingsDocker.mode")}
              value={mode}
              onChange={(m) => {
                setMode(m);
                setTest(null);
              }}
              options={[
                { value: "auto", label: t("settingsDocker.modeAuto") },
                { value: "custom", label: t("settingsDocker.modeCustom") },
              ]}
            />
          </div>
          <p className="mt-2 max-w-[600px] text-[12px] text-ink-3">{mode === "auto" ? t("settingsDocker.autoHint") : t("settingsDocker.customHint")}</p>

          {mode === "custom" && (
            <div className="mt-3">
              <label htmlFor="docker-socket" className="text-[12.5px] text-ink-2">
                {t("settingsDocker.socketPath")}
              </label>
              <input
                id="docker-socket"
                value={path}
                spellCheck={false}
                onChange={(e) => {
                  setPath(e.target.value);
                  setTest(null);
                }}
                placeholder="/run/user/1000/docker.sock"
                className="mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 font-mono text-[12.5px] outline-none hover:border-line-strong focus:border-beacon"
              />
              {missingPath && <p className="mt-1 text-[12px] text-err">{t("settingsDocker.pathRequired")}</p>}
            </div>
          )}

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <Button onClick={() => tester.mutate()} disabled={tester.isPending || missingPath}>
              {tester.isPending && <Spinner size={14} />}
              {tester.isPending ? t("settingsDocker.testing") : t("settingsDocker.test")}
            </Button>
            {dirty ? (
              <Button variant="primary" disabled={missingPath} onClick={() => void updateConfig({ docker: draft })}>
                {t("settingsDocker.save")}
              </Button>
            ) : (
              <Button onClick={() => void reconnect()} disabled={reconnecting}>
                {reconnecting && <Spinner size={14} />}
                {reconnecting ? t("conn.reconnecting") : t("conn.reconnect")}
              </Button>
            )}
          </div>
          {test && (
            <p role="status" className="selectable mt-3 flex items-start gap-2 text-[12.5px] [overflow-wrap:anywhere]">
              {test.result === "ok" ? (
                <>
                  <CheckCircle2 size={15} className="mt-px shrink-0 text-ok" />
                  {t("settingsDocker.testOk", { version: test.engine.version, path: test.endpoint.socketPath })}
                </>
              ) : (
                <>
                  <XCircle size={15} className="mt-px shrink-0 text-err" />
                  {t("settingsDocker.testFailed", { reason: testReason(test) })}
                </>
              )}
            </p>
          )}
        </div>

        <div className="px-5 py-4">
          <h3 className="mb-2 text-[13px] text-ink">{t("settingsDocker.current")}</h3>
          {state?.state === "connected" ? (
            <dl className="grid grid-cols-[auto_1fr] gap-x-5 gap-y-1 text-[12.5px]">
              <dt className="text-ink-3">{t("dashboard.socket")}</dt>
              <dd className="selectable font-mono text-[12px] [overflow-wrap:anywhere]">{state.endpoint.socketPath}</dd>
              <dt className="text-ink-3">{t("dashboard.source")}</dt>
              <dd>
                <SourceLabel source={state.endpoint.source} />
              </dd>
              <dt className="text-ink-3">{t("settingsDocker.engineVersion")}</dt>
              <dd className="tabular-nums">{state.engine.version}</dd>
              <dt className="text-ink-3">{t("settingsDocker.apiVersion")}</dt>
              <dd className="tabular-nums">{state.engine.apiVersion}</dd>
              <dt className="text-ink-3">{t("settingsDocker.rootless")}</dt>
              <dd>{state.engine.rootless ? t("diag.yes") : t("diag.no")}</dd>
            </dl>
          ) : (
            <p className="text-[12.5px] text-ink-3">{state?.state === "connecting" ? t("conn.connecting") : t("settingsDocker.none")}</p>
          )}
        </div>

        <div className="flex items-center justify-between gap-6 px-5 py-4">
          <div className="min-w-0">
            <div className="text-[13.5px]">{t("settingsDocker.compose")}</div>
            <p className="mt-0.5 text-[12px] text-ink-3">{t("settingsDocker.composeHint")}</p>
          </div>
          <div className="flex shrink-0 items-center gap-3">
            <span className="selectable text-[12.5px] tabular-nums">
              {compose.isFetching
                ? t("settingsDocker.composeChecking")
                : !compose.data
                  ? t("settingsDocker.composeNotChecked")
                  : compose.data.status === "available"
                    ? compose.data.version
                    : compose.data.status === "cliMissing"
                      ? t("settingsDocker.composeCliMissing")
                      : compose.data.status === "pluginMissing"
                        ? t("settingsDocker.composePluginMissing")
                        : t("settingsDocker.composeError", { message: compose.data.message })}
            </span>
            <Button size="sm" onClick={() => void compose.refetch()} disabled={compose.isFetching}>
              {t("settingsDocker.composeCheck")}
            </Button>
          </div>
        </div>

        <p className="flex gap-2 px-5 py-3 text-[12px] text-ink-3">
          <ShieldAlert size={14} className="mt-px shrink-0" />
          {t("settingsDocker.security")}
        </p>
      </div>
    </section>
  );
}
