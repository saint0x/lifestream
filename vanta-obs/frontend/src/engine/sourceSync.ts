import type { ObsRow } from "@/types";
import { text } from "@/types";

export interface SourceSyncState {
  readonly renderer: string;
  readonly permissionKind: string;
  readonly permissionRequired: boolean;
  readonly transport: string;
  readonly status: "ready" | "pending" | "blocked";
  readonly validationStatus: "ready" | "blocked";
  readonly issues: readonly string[];
  readonly obsKind: string;
}

export function sourceSyncState(source: ObsRow | null | undefined): SourceSyncState {
  const contract = objectValue(source?.source_contract_json);
  const permission = objectValue(source?.source_permission_json);
  const sync = objectValue(source?.source_sync_json);
  const validation = objectValue(source?.source_validation_json);
  const errors = arrayText(validation.errors);
  const warnings = arrayText(validation.warnings);

  return {
    renderer: stringValue(contract.renderer, text(source, "source_kind")),
    permissionKind: stringValue(permission.kind, "unknown"),
    permissionRequired: permission.required !== false,
    transport: stringValue(sync.transport, "pending"),
    status: syncStatus(stringValue(sync.status, text(source, "permission_state", "pending"))),
    validationStatus: validationStatus(stringValue(validation.status, "blocked")),
    issues: [...errors, ...warnings],
    obsKind: stringValue(contract.obs_kind, ""),
  };
}

export function sourceBadgeTone(source: ObsRow): "neutral" | "hd" | "premium" | "live" {
  const sync = sourceSyncState(source);
  if (sync.status === "ready" && sync.validationStatus === "ready") return "hd";
  if (sync.status === "blocked" || sync.validationStatus === "blocked") return "premium";
  return "neutral";
}

export function sourceSummary(source: ObsRow): string {
  const sync = sourceSyncState(source);
  const permission = sync.permissionRequired ? sync.permissionKind : "inline";
  return `${sync.renderer} / ${permission} / ${sync.transport}`;
}

export function sourceFilterSummary(filter: ObsRow): string {
  const contract = objectValue(filter.filter_contract_json);
  const kind = text(filter, "filter_kind");
  const obsKind = stringValue(contract.obs_kind, "native");
  return `${kind} / ${obsKind}`;
}

function objectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function stringValue(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim() ? value : fallback;
}

function arrayText(value: unknown): readonly string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((item): item is string => typeof item === "string" && item.trim().length > 0);
}

function syncStatus(value: string): SourceSyncState["status"] {
  if (value === "ready" || value === "blocked") return value;
  return "pending";
}

function validationStatus(value: string): SourceSyncState["validationStatus"] {
  return value === "ready" ? "ready" : "blocked";
}
