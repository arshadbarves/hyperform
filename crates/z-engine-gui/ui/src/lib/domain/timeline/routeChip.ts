import type { RouteInfo } from "../../protocol/RouteInfo";
import { MAIN_AGENT } from "../sessionView/types";

/** The words of a route chip: who was routed, to what, and why. */
export interface RouteChipText {
  lead: string;
  choice: string;
  reason: string;
}

export function routeChip(route: RouteInfo): RouteChipText {
  const parts: string[] = [];
  if (route.effort) parts.push(`${route.effort} effort`);
  if (route.model) parts.push(route.model);
  return {
    lead: route.agentId === MAIN_AGENT ? "Routed" : "Subagent routed",
    choice: parts.join(" · "),
    reason: route.reason,
  };
}
