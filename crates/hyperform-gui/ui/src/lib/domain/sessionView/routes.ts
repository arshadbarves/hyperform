import type { RouteInfo } from "../../protocol/RouteInfo";
import { capped, lastMessageId } from "./records";
import type { SessionView } from "./types";

const MAX_ROUTES = 50;

/** A routed choice, anchored after the message that was last when it arrived. */
export function addRoute(view: SessionView, route: RouteInfo, now: number): SessionView {
  const id = view.nextLocalId;
  const entry = { id, route, afterMessageId: lastMessageId(view.messages), at: now };
  return { ...view, routes: capped(view.routes, entry, MAX_ROUTES), nextLocalId: id + 1 };
}
