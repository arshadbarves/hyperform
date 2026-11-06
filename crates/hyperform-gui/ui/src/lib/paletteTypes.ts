import type { IconSvgElement } from "./ui/icons";

export type { IconSvgElement };

export interface PaletteItem {
  label: string;
  hint?: string;
  keywords: string;
  group?: string;
  icon?: IconSvgElement;
  shortcut?: string;
  run: () => void;
}
