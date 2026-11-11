/**
 * Revisión de la identidad del servidor (§4: huella en el primer uso,
 * confirmación explícita, cambio detectado con aviso de posible ataque).
 * Nunca se acepta sola: solo se guarda la huella que el usuario ha visto, y el
 * backend comprueba que es exactamente la que presenta el servidor.
 */
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { ShieldAlert, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, toCommandError } from "@/shared/ipc/client";
import type { RemoteProblem } from "@/shared/ipc/bindings/RemoteProblem";
import type { ServerConfig } from "@/shared/ipc/bindings/ServerConfig";
import { Button, CopyButton } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { hostPattern } from "./remote";
import { HostKeyBox } from "./ui";

type Problem = Extract<RemoteProblem, { kind: "hostKeyUnknown" | "hostKeyChanged" }>;

export function isIdentityProblem(p: RemoteProblem | null | undefined): p is Problem {
  return p?.kind === "hostKeyUnknown" || p?.kind === "hostKeyChanged";
}

export function IdentityReview({ config, problem, onTrusted }: { config: ServerConfig; problem: Problem; onTrusted: () => void }) {
  const { t } = useTranslation();
  const changed = problem.kind === "hostKeyChanged";
  const [checked, setChecked] = useState(false);
  const trust = useMutation({
    mutationFn: () => api.serverTrust(config, problem.presented!, changed),
    onSuccess: onTrusted,
  });
  const err = trust.error ? toCommandError(trust.error) : null;

  return (
    <section
      data-testid="identity-review"
      aria-label={t(changed ? "servers.hostKey.changedTitle" : "servers.hostKey.unknownTitle")}
      className={changed ? "space-y-3 rounded-lg border-2 border-err/60 p-4" : "space-y-3 rounded-lg border border-beacon/40 p-4"}
    >
      <h3 className={changed ? "flex items-center gap-2 text-[14.5px] font-semibold text-err" : "flex items-center gap-2 text-[14.5px] font-semibold"}>
        {changed ? <ShieldAlert size={17} aria-hidden /> : <ShieldCheck size={17} aria-hidden className="text-beacon-strong" />}
        {t(changed ? "servers.hostKey.changedTitle" : "servers.hostKey.unknownTitle")}
      </h3>
      <p className="text-[13px] text-ink-2">
        {t(changed ? "servers.hostKey.changedBody" : "servers.hostKey.unknownBody", { host: config.host, port: config.port })}
      </p>
      {problem.presented ? (
        <HostKeyBox info={problem.presented} tone={changed ? "danger" : "neutral"} />
      ) : (
        <Banner tone="error">{t("servers.hostKey.noKey")}</Banner>
      )}
      <div className="rounded-md bg-surface-2 px-3 py-2 text-[12.5px]">
        <p className="text-ink-2">{t("servers.hostKey.howToVerify")}</p>
        <div className="mt-1 flex items-center gap-2 rounded bg-code px-2 py-1">
          <code className="selectable min-w-0 flex-1 font-mono text-[12px]">ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</code>
          <CopyButton text="ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub" compact />
        </div>
      </div>

      {changed && problem.kind === "hostKeyChanged" && problem.inUserFile ? (
        <div className="space-y-2">
          <Banner tone="warn">
            {t("servers.hostKey.inUserFile", {
              file: problem.file ?? "~/.ssh/known_hosts",
            })}
          </Banner>
          <div className="flex items-center gap-2 rounded-md bg-code px-2.5 py-1.5">
            <code className="selectable min-w-0 flex-1 font-mono text-[12.5px]">{`ssh-keygen -R '${hostPattern(config)}'`}</code>
            <CopyButton text={`ssh-keygen -R '${hostPattern(config)}'`} compact />
          </div>
        </div>
      ) : (
        <>
          {changed && <Checkbox checked={checked} onChange={setChecked} label={t("servers.hostKey.confirmChanged")} />}
          {err && <Banner tone="error">{err.code === "server.hostKeyMismatch" ? t("servers.hostKey.mismatch") : err.message}</Banner>}
          <div className="flex justify-end">
            <Button
              variant={changed ? "danger" : "primary"}
              disabled={!problem.presented || (changed && !checked) || trust.isPending}
              onClick={() => trust.mutate()}
            >
              {trust.isPending && <Spinner size={14} />}
              {t(changed ? "servers.hostKey.replace" : "servers.hostKey.trust")}
            </Button>
          </div>
        </>
      )}
    </section>
  );
}
