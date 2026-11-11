/**
 * Frase de paso o contraseña para conectar (0.10.0, D-077). El valor solo
 * viaja en la llamada de conexión; no se guarda en el estado de la interfaz
 * más que mientras el diálogo está abierto. «Recordar» lo guarda en el
 * keyring del sistema; si no hay keyring, se dice y no se ofrece.
 */
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { KeyRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useServers } from "@/app/server";
import { api } from "@/shared/ipc/client";
import { Button } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog } from "@/shared/ui/feedback";
import { displayAddress } from "./remote";

export function SecretDialog() {
  const prompt = useServers((s) => s.secretPrompt);
  if (!prompt) return null;
  return <SecretForm key={`${prompt.server}-${prompt.kind}-${prompt.retry}`} />;
}

function SecretForm() {
  const { t } = useTranslation();
  const prompt = useServers((s) => s.secretPrompt)!;
  const server = useServers((s) => s.servers.find((x) => x.id === prompt.server));
  const { connect, closeSecretPrompt } = useServers.getState();
  const [secret, setSecret] = useState("");
  const [remember, setRemember] = useState(false);
  const keyring = useQuery({
    queryKey: ["keyringStatus"],
    queryFn: api.keyringStatus,
    staleTime: 60_000,
  });
  const keyringOk = keyring.data?.state === "available";
  useEffect(() => {
    if (keyring.data && !keyringOk) setRemember(false);
  }, [keyring.data, keyringOk]);
  const c = server?.config;
  if (!c) return null;
  const password = prompt.kind === "password";

  const submit = () => {
    if (!secret) return;
    const value = secret;
    setSecret("");
    void connect(prompt.server, value, remember && keyringOk);
  };

  return (
    <Dialog
      target={false}
      title={t(password ? "servers.secret.passwordTitle" : "servers.secret.passphraseTitle", { name: c.name })}
      onClose={closeSecretPrompt}
      footer={
        <>
          <Button onClick={closeSecretPrompt}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={!secret} onClick={submit}>
            {t("servers.connect")}
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        className="space-y-3"
      >
        <p className="flex items-center gap-2 text-[13px] text-ink-2">
          <KeyRound size={15} aria-hidden className="text-ink-3" />
          <span className="selectable font-mono text-[12.5px]">{displayAddress(c)}</span>
        </p>
        {prompt.retry && <Banner tone="error">{t(password ? "servers.secret.wrongPassword" : "servers.secret.wrongPassphrase")}</Banner>}
        {password && <Banner tone="warn">{t("servers.secret.passwordDiscouraged")}</Banner>}
        <label className="block">
          <span className="text-[12.5px] font-medium text-ink-2">
            {t(password ? "servers.secret.password" : "servers.secret.passphrase", { path: c.keyPath })}
          </span>
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            className="mt-1 h-9 w-full rounded-md border border-line bg-surface px-2.5 text-[13.5px] focus:border-beacon focus:outline-none"
          />
        </label>
        <Checkbox
          checked={remember && keyringOk}
          disabled={!keyringOk}
          onChange={setRemember}
          label={t("servers.secret.remember")}
          hint={keyring.data && !keyringOk ? t("servers.secret.noKeyring") : t("servers.secret.rememberHint")}
        />
      </form>
    </Dialog>
  );
}
