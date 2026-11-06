/** The kit's one button vocabulary; `Button.svelte` and Bits UI triggers both use it. */

export type ButtonVariant = "ghost" | "secondary" | "primary" | "accent" | "danger" | "icon" | "icon-mini";
export type ButtonSize = "s" | "m" | "l";

export interface ButtonLook {
  size?: ButtonSize;
  /** A filled danger button, for the one destructive step of a confirm. */
  solid?: boolean;
  /** An icon button showing that its panel or mode is on. */
  active?: boolean;
  /** The icon turns, while a refresh runs. */
  spinning?: boolean;
  className?: string;
}

const VARIANT_CLASS: Record<ButtonVariant, string> = {
  ghost: "btn-ghost",
  secondary: "btn-secondary",
  primary: "btn-primary",
  accent: "btn-accent",
  danger: "btn-danger",
  icon: "icon-btn",
  "icon-mini": "icon-btn-mini",
};

export function buttonClass(variant: ButtonVariant = "ghost", look: ButtonLook = {}): string {
  const icon = variant === "icon" || variant === "icon-mini";
  const parts = [VARIANT_CLASS[variant]];
  if (!icon && look.size && look.size !== "m") parts.push(`size-${look.size}`);
  if (look.solid && variant === "danger") parts.push("is-solid");
  if (look.active) parts.push("is-active");
  if (look.spinning) parts.push("spinning");
  if (look.className) parts.push(look.className);
  return parts.join(" ");
}
