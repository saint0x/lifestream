import type { ObsRow } from "@/types";
import { boolish, text } from "@/types";

export interface KeyLike {
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
  readonly code: string;
  readonly target?: EventTarget | null;
}

export function eventBinding(event: KeyLike): string {
  return [
    event.ctrlKey ? "Ctrl" : "",
    event.altKey ? "Alt" : "",
    event.shiftKey ? "Shift" : "",
    event.metaKey ? "Meta" : "",
    event.code,
  ].filter(Boolean).join("+");
}

export function matchingHotkey(hotkeys: readonly ObsRow[], event: KeyLike): ObsRow | null {
  if (isEditableTarget(event.target)) return null;
  const binding = eventBinding(event);
  return hotkeys.find((hotkey) => boolish(hotkey, "enabled") && text(hotkey, "binding") === binding) ?? null;
}

function isEditableTarget(target: EventTarget | null | undefined): boolean {
  if (typeof HTMLElement === "undefined") return false;
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select";
}
