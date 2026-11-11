/**
 * Asistente «Añadir servidor» y edición de un servidor (§3.2). Cuatro pasos:
 * datos generales, autenticación, Docker y prueba de conexión. La clave
 * privada nunca se copia: solo se guarda su ruta. Los secretos no se guardan
 * con el perfil (van al keyring si el usuario lo pide al conectar).
 */
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, Plug } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useServers } from "@/app/server";
import { api, toCommandError } from "@/shared/ipc/client";
import type { ProfileIssue } from "@/shared/ipc/bindings/ProfileIssue";
import type { ServerAuthMethod } from "@/shared/ipc/bindings/ServerAuthMethod";
import type { ServerConfig } from "@/shared/ipc/bindings/ServerConfig";
import type { ServerDockerTarget } from "@/shared/ipc/bindings/ServerDockerTarget";
import type { TestReport } from "@/shared/ipc/bindings/TestReport";
import { Button, Segmented, Switch, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { isIdentityProblem, IdentityReview } from "./IdentityReview";
import { useServersNav } from "./nav";
import { emptyServer } from "./remote";
import { CheckList, RemoteProblemView } from "./ui";

type Step = "general" | "auth" | "docker" | "test";
const STEPS: Step[] = ["general", "auth", "docker", "test"];
const FIELD_STEP: Record<string, Step> = {
  name: "general",
  host: "general",
  port: "general",
  user: "general",
  description: "general",
  keyPath: "auth",
  dockerSocket: "docker",
  dockerContext: "docker",
  connectTimeout: "docker",
  proxyJump: "docker",
};

function Field({ label, hint, issue, children }: { label: string; hint?: string; issue?: string | null; children: ReactNode }) {
  return (
    <label className="block">
      <span className="text-[12.5px] font-medium text-ink-2">{label}</span>
      <div className="mt-1">{children}</div>
      {issue ? (
        <span className="mt-1 block text-[12px] text-err">{issue}</span>
      ) : hint ? (
        <span className="mt-1 block text-[12px] text-ink-3">{hint}</span>
      ) : null}
    </label>
  );
}

const input = "h-9 w-full rounded-md border border-line bg-surface px-2.5 text-[13.5px] focus:border-beacon focus:outline-none aria-[invalid=true]:border-err";

export function ServerEditor({ id }: { id: string | null }) {
  const { t } = useTranslation();
  const nav = useServersNav();
  const existing = useServers((s) => (id ? s.servers.find((x) => x.id === id)?.config : null));
  const load = useServers((s) => s.load);
  const connect = useServers((s) => s.connect);
  const [c, setC] = useState<ServerConfig>(() => existing ?? emptyServer());
  const [step, setStep] = useState<Step>("general");
  const [issues, setIssues] = useState<ProfileIssue[]>([]);
  const [touched, setTouched] = useState<Set<string>>(new Set());
  const [secret, setSecret] = useState("");
  const [report, setReport] = useState<TestReport | null>(null);
  const set = <K extends keyof ServerConfig>(k: K, v: ServerConfig[K]) => {
    setC((x) => ({ ...x, [k]: v }));
    setTouched((s) => new Set(s).add(k));
    setReport(null);
  };

  // Validación del backend (las mismas reglas que al guardar y al conectar).
  const seq = useRef(0);
  useEffect(() => {
    const n = ++seq.current;
    const timer = window.setTimeout(() => {
      void api
        .serverValidate(c)
        .then((r) => n === seq.current && setIssues(r))
        .catch(() => undefined);
    }, 200);
    return () => window.clearTimeout(timer);
  }, [c]);
  const issueOf = (field: string, always = false) => {
    const i = issues.find((x) => x.field === field);
    return i && (always || touched.has(field)) ? t(`servers.issues.${i.code}`) : null;
  };
  const stepHasIssues = (s: Step) => issues.some((i) => (FIELD_STEP[i.field] ?? "general") === s);

  const secretStatus = useQuery({
    queryKey: ["serverSecret", id],
    queryFn: () => api.serverSecretStatus(id!),
    enabled: !!id && !!existing && (existing.auth === "keyPassphrase" || existing.auth === "password"),
  });
  const forget = useMutation({
    mutationFn: () => api.serverForgetSecret(id!),
    onSuccess: () => void secretStatus.refetch(),
  });

  const needsSecret = c.auth === "keyPassphrase" || c.auth === "password";
  const test = useMutation({
    mutationFn: () => api.serverTest(c, needsSecret && secret ? secret : null),
    onSuccess: (r) => setReport(r),
  });
  const save = useMutation({
    mutationFn: (andConnect: boolean) => api.serverSave(c).then((saved) => ({ saved, andConnect })),
    onSuccess: async ({ saved, andConnect }) => {
      setSecret("");
      await load();
      nav.showList();
      if (andConnect) void connect(saved.id);
    },
  });
  const saveError = save.error ? toCommandError(save.error) : null;
  const valid = issues.length === 0;

  const pickKey = async () => {
    const p = await api.pickFile(t("servers.editor.pickKey"), "~/.ssh");
    if (p) set("keyPath", p);
  };

  const authOptions = useMemo(
    () =>
      (["agent", "key", "keyPassphrase", "password"] as ServerAuthMethod[]).map((v) => ({
        value: v,
        label: t(`servers.auth.${v}`),
      })),
    [t],
  );
  const targetOptions = useMemo(
    () =>
      (["default", "socket", "context"] as ServerDockerTarget[]).map((v) => ({
        value: v,
        label: t(`servers.docker.${v}`),
      })),
    [t],
  );
  const problem = report?.problem ?? null;
  const allTouched = () => setTouched(new Set(Object.keys(FIELD_STEP)));

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("servers.back")}
        </Button>
        <h2 className="text-[15px] font-semibold">{existing ? t("servers.editor.editTitle", { name: existing.name }) : t("servers.editor.addTitle")}</h2>
      </div>

      <div className="mx-auto w-full max-w-[760px] px-8 py-6">
        <nav aria-label={t("servers.editor.steps")} className="mb-6">
          <ol className="flex flex-wrap gap-2">
            {STEPS.map((s, i) => (
              <li key={s}>
                <button
                  type="button"
                  aria-current={step === s ? "step" : undefined}
                  onClick={() => setStep(s)}
                  className={cx(
                    "inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[12.5px]",
                    step === s ? "border-beacon bg-beacon-soft text-beacon-strong" : "border-line text-ink-2 hover:border-line-strong",
                  )}
                >
                  <span className="tabular-nums">{i + 1}</span>
                  {t(`servers.editor.step.${s}`)}
                  {s !== "test" && stepHasIssues(s) && [...touched].some((f) => (FIELD_STEP[f] ?? "") === s) && (
                    <span aria-hidden className="h-1.5 w-1.5 rounded-full bg-err" />
                  )}
                </button>
              </li>
            ))}
          </ol>
        </nav>

        {step === "general" && (
          <div className="grid gap-4 sm:grid-cols-[1fr_120px]">
            <div className="sm:col-span-2">
              <Field label={t("servers.fields.name")} issue={issueOf("name")} hint={t("servers.hints.name")}>
                <input className={input} value={c.name} aria-invalid={!!issueOf("name")} onChange={(e) => set("name", e.target.value)} />
              </Field>
            </div>
            <Field label={t("servers.fields.host")} issue={issueOf("host")} hint={t("servers.hints.host")}>
              <input
                className={cx(input, "font-mono")}
                value={c.host}
                spellCheck={false}
                aria-invalid={!!issueOf("host")}
                onChange={(e) => set("host", e.target.value)}
              />
            </Field>
            <Field label={t("servers.fields.port")} issue={issueOf("port")}>
              <input
                className={cx(input, "tabular-nums")}
                type="number"
                min={1}
                max={65535}
                value={c.port}
                aria-invalid={!!issueOf("port")}
                onChange={(e) => set("port", Number(e.target.value) || 0)}
              />
            </Field>
            <div className="sm:col-span-2">
              <Field label={t("servers.fields.user")} issue={issueOf("user")} hint={t("servers.hints.user")}>
                <input
                  className={cx(input, "font-mono")}
                  value={c.user}
                  spellCheck={false}
                  autoComplete="off"
                  aria-invalid={!!issueOf("user")}
                  onChange={(e) => set("user", e.target.value)}
                />
              </Field>
            </div>
            <div className="sm:col-span-2">
              <Field label={t("servers.fields.description")} issue={issueOf("description")}>
                <textarea className={cx(input, "h-20 py-2")} value={c.description} onChange={(e) => set("description", e.target.value)} />
              </Field>
            </div>
          </div>
        )}

        {step === "auth" && (
          <div className="space-y-4">
            <div>
              <p className="mb-1.5 text-[12.5px] font-medium text-ink-2">{t("servers.fields.auth")}</p>
              <Segmented label={t("servers.fields.auth")} value={c.auth} options={authOptions} onChange={(v) => set("auth", v)} />
              <p className="mt-2 text-[12.5px] text-ink-3">{t(`servers.authHint.${c.auth}`)}</p>
            </div>
            {(c.auth === "key" || c.auth === "keyPassphrase") && (
              <Field label={t("servers.fields.keyPath")} issue={issueOf("keyPath", true)} hint={t("servers.hints.keyPath")}>
                <div className="flex gap-2">
                  <input
                    className={cx(input, "font-mono")}
                    value={c.keyPath}
                    spellCheck={false}
                    placeholder="~/.ssh/id_ed25519"
                    onChange={(e) => set("keyPath", e.target.value)}
                  />
                  <Button onClick={() => void pickKey()}>{t("servers.editor.choose")}</Button>
                </div>
              </Field>
            )}
            {c.auth === "password" && <Banner tone="warn">{t("servers.secret.passwordDiscouraged")}</Banner>}
            {(c.auth === "keyPassphrase" || c.auth === "password") && (
              <div className="rounded-lg border border-line bg-surface-2 px-3.5 py-3 text-[12.5px] text-ink-2">
                <p>{t("servers.editor.secretWhen")}</p>
                {secretStatus.data && (
                  <div className="mt-2 flex flex-wrap items-center gap-3">
                    <span>
                      {secretStatus.data.keyring.state === "unavailable"
                        ? t("servers.secret.noKeyring")
                        : secretStatus.data.stored
                          ? t("servers.editor.secretStored")
                          : t("servers.editor.secretNotStored")}
                    </span>
                    {secretStatus.data.stored && (
                      <Button size="sm" onClick={() => forget.mutate()} disabled={forget.isPending}>
                        {t("servers.forgetSecret")}
                      </Button>
                    )}
                  </div>
                )}
              </div>
            )}
            <p className="text-[12px] text-ink-3">{t("servers.editor.keyNeverCopied")}</p>
          </div>
        )}

        {step === "docker" && (
          <div className="space-y-4">
            <div>
              <p className="mb-1.5 text-[12.5px] font-medium text-ink-2">{t("servers.fields.dockerTarget")}</p>
              <Segmented label={t("servers.fields.dockerTarget")} value={c.dockerTarget} options={targetOptions} onChange={(v) => set("dockerTarget", v)} />
              <p className="mt-2 text-[12.5px] text-ink-3">{t(`servers.dockerHint.${c.dockerTarget}`)}</p>
            </div>
            {c.dockerTarget === "socket" && (
              <Field label={t("servers.fields.dockerSocket")} issue={issueOf("dockerSocket", true)}>
                <input
                  className={cx(input, "font-mono")}
                  value={c.dockerSocket}
                  spellCheck={false}
                  placeholder="/run/user/1000/docker.sock"
                  onChange={(e) => set("dockerSocket", e.target.value)}
                />
              </Field>
            )}
            {c.dockerTarget === "context" && (
              <Field label={t("servers.fields.dockerContext")} issue={issueOf("dockerContext", true)}>
                <input className={cx(input, "font-mono")} value={c.dockerContext} spellCheck={false} onChange={(e) => set("dockerContext", e.target.value)} />
              </Field>
            )}
            <details className="rounded-lg border border-line px-3.5 py-2.5">
              <summary className="cursor-pointer text-[13px] font-medium">{t("servers.editor.advanced")}</summary>
              <div className="mt-3 grid gap-4 sm:grid-cols-2">
                <Field label={t("servers.fields.connectTimeout")} issue={issueOf("connectTimeout", true)} hint={t("servers.hints.connectTimeout")}>
                  <input
                    className={cx(input, "tabular-nums")}
                    type="number"
                    min={3}
                    max={120}
                    value={c.connectTimeout}
                    onChange={(e) => set("connectTimeout", Number(e.target.value) || 0)}
                  />
                </Field>
                <Field label={t("servers.fields.proxyJump")} issue={issueOf("proxyJump", true)} hint={t("servers.hints.proxyJump")}>
                  <input
                    className={cx(input, "font-mono")}
                    value={c.proxyJump}
                    spellCheck={false}
                    placeholder="usuario@bastion:22"
                    onChange={(e) => set("proxyJump", e.target.value)}
                  />
                </Field>
              </div>
            </details>
            <Switch checked={c.autoConnect} onChange={(v) => set("autoConnect", v)} label={t("servers.fields.autoConnect")} />
          </div>
        )}

        {step === "test" && (
          <div className="space-y-4">
            <p className="text-[13px] text-ink-2">{t("servers.test.intro")}</p>
            {needsSecret && (
              <Field
                label={t(c.auth === "password" ? "servers.secret.password" : "servers.secret.passphrase", { path: c.keyPath })}
                hint={t("servers.test.secretHint")}
              >
                <input className={input} type="password" autoComplete="off" value={secret} onChange={(e) => setSecret(e.target.value)} />
              </Field>
            )}
            <div className="flex items-center gap-3">
              <Button
                variant="primary"
                disabled={!valid || test.isPending}
                onClick={() => {
                  allTouched();
                  test.mutate();
                }}
              >
                {test.isPending ? <Spinner size={14} /> : <Plug size={14} />}
                {test.isPending ? t("servers.test.running") : t("servers.test.run")}
              </Button>
              {report && !problem && report.engine && (
                <span role="status" className="text-[13px] text-ok">
                  {t("servers.test.ok", {
                    version: report.engine.version,
                    api: report.engine.apiVersion,
                    latency: report.latencyMs ?? "?",
                  })}
                </span>
              )}
            </div>
            {!valid && <Banner tone="warn">{t("servers.editor.fixFirst")}</Banner>}
            {test.error && <Banner tone="error">{toCommandError(test.error).message}</Banner>}
            {report && <CheckList steps={report.steps} />}
            {report && problem && isIdentityProblem(problem) && <IdentityReview config={c} problem={problem} onTrusted={() => test.mutate()} />}
            {report && problem && !isIdentityProblem(problem) && <RemoteProblemView problem={problem} config={c} />}
            <p className="text-[12px] text-ink-3">{t("servers.test.readOnly")}</p>
          </div>
        )}

        {saveError && (
          <div className="mt-5">
            <Banner tone="error">{saveError.code === "server.invalid" ? t("servers.editor.fixFirst") : saveError.message}</Banner>
          </div>
        )}

        <div className="mt-8 flex flex-wrap items-center gap-2 border-t border-line pt-4">
          <Button onClick={nav.showList}>{t("common.cancel")}</Button>
          <div className="ml-auto flex gap-2">
            {step !== "general" && <Button onClick={() => setStep(STEPS[STEPS.indexOf(step) - 1]!)}>{t("servers.editor.previous")}</Button>}
            {step !== "test" && (
              <Button
                onClick={() => {
                  allTouched();
                  setStep(STEPS[STEPS.indexOf(step) + 1]!);
                }}
              >
                {t("servers.editor.next")}
              </Button>
            )}
            <Button
              disabled={!valid || save.isPending}
              onClick={() => {
                allTouched();
                save.mutate(false);
              }}
            >
              {t("servers.editor.save")}
            </Button>
            <Button
              variant="primary"
              disabled={!valid || save.isPending}
              onClick={() => {
                allTouched();
                save.mutate(true);
              }}
            >
              {t("servers.editor.saveConnect")}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
