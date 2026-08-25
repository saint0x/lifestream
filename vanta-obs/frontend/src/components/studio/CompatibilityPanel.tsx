import { Download, RefreshCcw, Upload } from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import type { ObsBridgeConnection, ObsExportJob, ObsImportReport } from "@/types";
import { jsonArray, text, type ObsRow } from "@/types";
import { Panel } from "./Panel";

export function CompatibilityPanel({
  importReport,
  exportJob,
  bridgeConnection,
  busy,
  onImportFile,
  onExport,
  onSync,
}: {
  readonly importReport: ObsImportReport | null;
  readonly exportJob: ObsExportJob | null;
  readonly bridgeConnection: ObsBridgeConnection | null;
  readonly busy: boolean;
  readonly onImportFile: (file: File) => void;
  readonly onExport: () => void;
  readonly onSync: () => void;
}) {
  const exportWarnings = jsonArray(exportJob, "warnings_json");
  const reportJson = importReport?.report_json;
  const importWarnings = isRow(reportJson) ? jsonArray(reportJson, "warnings") : [];
  const warningCount = importWarnings.length + exportWarnings.length;
  const syncStatus = bridgeConnection ? text(bridgeConnection, "sync_status") : exportJob ? text(exportJob, "status") : importReport ? text(importReport, "status") : "standby";

  return (
    <Panel
      title="OBS"
      icon={<RefreshCcw />}
      summary={<><strong>{syncStatus}</strong><span>{warningCount} warn</span></>}
      defaultCollapsed
    >
      <div className="obs-compat">
        <div className="obs-compat__actions">
          <Input
            type="file"
            accept="application/json,.json"
            disabled={busy}
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              if (file) onImportFile(file);
              event.currentTarget.value = "";
            }}
          />
          <Button size="sm" variant="secondary" icon={<Download />} onClick={onExport} disabled={busy}>
            Export
          </Button>
          <Button size="sm" variant="secondary" icon={<Upload />} onClick={onSync} disabled={busy}>
            Sync
          </Button>
        </div>
        {importReport ? (
          <div className="obs-kv mono">
            <span>Import</span>
            <Badge tone={text(importReport, "status") === "ready" ? "hd" : "premium"}>
              {text(importReport, "status")}
            </Badge>
          </div>
        ) : null}
        {exportJob ? (
          <div className="obs-kv mono">
            <span>Export</span>
            <Badge tone="hd">{text(exportJob, "status")}</Badge>
          </div>
        ) : null}
        {bridgeConnection ? (
          <div className="obs-kv mono">
            <span>Bridge</span>
            <Badge tone={text(bridgeConnection, "sync_status") === "synced" ? "hd" : "premium"}>
              {text(bridgeConnection, "sync_status")}
            </Badge>
          </div>
        ) : null}
        {[...importWarnings, ...exportWarnings].slice(0, 3).map((warning, index) => (
          <div className="obs-event" key={`${text(warning, "code")}-${index}`}>
            <RefreshCcw size={13} />
            <span>{text(warning, "code") || text(warning, "detail")}</span>
          </div>
        ))}
      </div>
    </Panel>
  );
}

function isRow(value: unknown): value is ObsRow {
  return typeof value === "object" && value !== null && "id" in value === false;
}
