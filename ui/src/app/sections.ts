import {
  Boxes,
  Cable,
  FolderSymlink,
  HardDrive,
  Layers,
  LayoutDashboard,
  type LucideIcon,
  Radio,
  Settings,
  Workflow,
} from "lucide-react";

export type SectionId =
  | "dashboard"
  | "containers"
  | "images"
  | "networks"
  | "volumes"
  | "binds"
  | "compose"
  | "events"
  | "settings";

export interface SectionDef {
  id: SectionId;
  icon: LucideIcon;
  group: "top" | "resources" | "orchestration" | "monitoring" | "bottom";
}

/** Orden exacto exigido por el documento de requisitos (§3.A). */
export const SECTIONS: SectionDef[] = [
  { id: "dashboard", icon: LayoutDashboard, group: "top" },
  { id: "containers", icon: Boxes, group: "resources" },
  { id: "images", icon: Layers, group: "resources" },
  { id: "networks", icon: Cable, group: "resources" },
  { id: "volumes", icon: HardDrive, group: "resources" },
  { id: "binds", icon: FolderSymlink, group: "resources" },
  { id: "compose", icon: Workflow, group: "orchestration" },
  { id: "events", icon: Radio, group: "monitoring" },
  { id: "settings", icon: Settings, group: "bottom" },
];
