import { useTranslation } from "react-i18next";
import type { SectionId } from "@/app/sections";
import { SECTIONS } from "@/app/sections";

/**
 * Apartado que aún no está implementado. No muestra datos ni acciones:
 * solo explica qué hará y en qué fase llega, para no simular funcionalidad.
 */
export function PendingSection({ id }: { id: Exclude<SectionId, "settings"> }) {
  const { t } = useTranslation();
  const Icon = SECTIONS.find((s) => s.id === id)!.icon;
  const planned = t(`sections.${id}.planned`, { returnObjects: true }) as string[];
  const phase = t(`sections.${id}.phase`);

  return (
    <div className="mx-auto mt-10 max-w-[640px] px-8">
      <div className="rounded-xl border border-dashed border-line-strong bg-surface px-8 py-8">
        <div className="flex items-start gap-4">
          <div className="grid h-11 w-11 shrink-0 place-items-center rounded-lg bg-beacon-soft text-beacon-strong">
            <Icon size={22} strokeWidth={1.75} />
          </div>
          <div className="min-w-0">
            <p className="text-[12.5px] font-medium text-beacon-strong">{t("pending.phase", { phase })}</p>
            <p className="mt-2 text-[13.5px] text-ink-2">{t("pending.intro")}</p>
            <ul className="mt-3 space-y-1.5 text-[13.5px]">
              {planned.map((p) => (
                <li key={p} className="flex gap-2.5">
                  <span aria-hidden className="mt-[9px] h-1 w-1 shrink-0 rounded-full bg-ink-3" />
                  <span>{p}</span>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}
