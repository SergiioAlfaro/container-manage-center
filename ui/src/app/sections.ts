import {
  Boxes,
  Cable,
  DatabaseBackup,
  FolderSymlink,
  HardDrive,
  Activity,
  Layers,
  LayoutDashboard,
  type LucideIcon,
  Radio,
  Server,
  Settings,
  ShieldCheck,
  SquareTerminal,
  Workflow,
} from "lucide-react";

export type SectionId =
  | "dashboard"
  | "servers"
  | "containers"
  | "images"
  | "networks"
  | "volumes"
  | "binds"
  | "compose"
  | "terminals"
  | "audit"
  | "backup"
  | "monitor"
  | "events"
  | "settings";

export interface SectionDef {
  id: SectionId;
  icon: LucideIcon;
  group: "top" | "resources" | "orchestration" | "tools" | "monitoring" | "bottom";
}

/** Orden exacto exigido por el documento de requisitos (§3.A); «Terminales»
 *  (0.9.1) va en su propio grupo, «Herramientas», antes de la supervisión. */
export const SECTIONS: SectionDef[] = [
  { id: "dashboard", icon: LayoutDashboard, group: "top" },
  // 0.10.0: administración remota y varios servidores.
  { id: "servers", icon: Server, group: "top" },
  { id: "containers", icon: Boxes, group: "resources" },
  { id: "images", icon: Layers, group: "resources" },
  { id: "networks", icon: Cable, group: "resources" },
  { id: "volumes", icon: HardDrive, group: "resources" },
  { id: "binds", icon: FolderSymlink, group: "resources" },
  { id: "compose", icon: Workflow, group: "orchestration" },
  { id: "terminals", icon: SquareTerminal, group: "tools" },
  // E3 (1.3.0): auditoría de seguridad.
  { id: "audit", icon: ShieldCheck, group: "tools" },
  // E4 (1.4.0): copias de seguridad y restauración.
  { id: "backup", icon: DatabaseBackup, group: "tools" },
  // E5 (1.5.0): monitorización avanzada.
  { id: "monitor", icon: Activity, group: "monitoring" },
  { id: "events", icon: Radio, group: "monitoring" },
  { id: "settings", icon: Settings, group: "bottom" },
];
