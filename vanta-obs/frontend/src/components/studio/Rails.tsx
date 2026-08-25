import { useState } from "react";
import { ArrowDown, ArrowUp, Copy, Plus, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { orderedScenes, type SceneMoveDirection } from "@/engine/scenes";
import { sourceBadgeTone, sourceSummary, sourceSyncState } from "@/engine/sourceSync";
import type { ObsRow } from "@/types";
import { boolish, text, num } from "@/types";

export function SceneList({
  scenes,
  templates,
  activeId,
  selectedId,
  onSelect,
  onDuplicate,
  onMove,
  onDelete,
  onCreateFromTemplate,
}: {
  readonly scenes: readonly ObsRow[];
  readonly templates: readonly ObsRow[];
  readonly activeId: string;
  readonly selectedId: string;
  readonly onSelect: (id: string) => void;
  readonly onDuplicate: (id: string) => void;
  readonly onMove: (id: string, direction: SceneMoveDirection) => void;
  readonly onDelete: (id: string) => void;
  readonly onCreateFromTemplate: (templateId: string) => void;
}) {
  const ordered = orderedScenes(scenes);
  const [templateId, setTemplateId] = useState(templates[0]?.id ?? "");
  return (
    <div className="obs-list">
      {templates.length > 0 ? (
        <div className="obs-template-bar">
          <select
            className="obs-template-bar__select mono"
            aria-label="Scene template"
            value={templateId}
            onChange={(event) => setTemplateId(event.target.value)}
          >
            {templates.map((template) => (
              <option key={template.id} value={template.id}>
                {text(template, "label")}
              </option>
            ))}
          </select>
          <Button
            size="sm"
            variant="secondary"
            icon={<Plus />}
            onClick={() => templateId && onCreateFromTemplate(templateId)}
          >
            Add
          </Button>
        </div>
      ) : null}
      {ordered.map((scene, index) => {
        const deleteBlocked = scene.id === activeId || boolish(scene, "locked") || ordered.length <= 1;
        const validation = scene.scene_validation_json as ObsRow | undefined;
        const validationStatus = text(validation, "status", "unknown");
        const validationTone = validationStatus === "ready" ? "hd" : validationStatus === "warning" ? "neutral" : "premium";
        return (
        <div
          key={scene.id}
          className={`obs-list__row ${scene.id === selectedId ? "is-selected" : ""}`}
          role="button"
          tabIndex={0}
          onClick={() => onSelect(scene.id)}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") onSelect(scene.id);
          }}
        >
          <span className="obs-list__meta">
            <strong>{text(scene, "name")}</strong>
            <em className="mono">
              {text(scene, "transition_kind")} / {num(scene, "transition_duration_ms")}ms
            </em>
          </span>
          <span className="obs-list__badges">
            <Badge tone={validationTone}>{validationStatus}</Badge>
            {scene.id === activeId ? <Badge tone="live">PGM</Badge> : null}
          </span>
          <span className="obs-list__actions">
            <button
              type="button"
              className="obs-icon-button"
              title="Move scene up"
              aria-label="Move scene up"
              disabled={index === 0}
              onClick={(event) => {
                event.stopPropagation();
                onMove(scene.id, "up");
              }}
            >
              <ArrowUp />
            </button>
            <button
              type="button"
              className="obs-icon-button"
              title="Move scene down"
              aria-label="Move scene down"
              disabled={index === ordered.length - 1}
              onClick={(event) => {
                event.stopPropagation();
                onMove(scene.id, "down");
              }}
            >
              <ArrowDown />
            </button>
            <button
              type="button"
              className="obs-icon-button"
              title="Duplicate scene"
              aria-label="Duplicate scene"
              onClick={(event) => {
                event.stopPropagation();
                onDuplicate(scene.id);
              }}
            >
              <Copy />
            </button>
            <button
              type="button"
              className="obs-icon-button"
              title="Delete scene"
              aria-label="Delete scene"
              disabled={deleteBlocked}
              onClick={(event) => {
                event.stopPropagation();
                onDelete(scene.id);
              }}
            >
              <Trash2 />
            </button>
          </span>
        </div>
      );
      })}
    </div>
  );
}

export function SourceList({
  sources,
  selectedId,
  onSelect,
}: {
  readonly sources: readonly ObsRow[];
  readonly selectedId: string;
  readonly onSelect: (id: string) => void;
}) {
  return (
    <div className="obs-list">
      {sources.map((source) => (
        <button
          key={source.id}
          className={`obs-list__row ${source.id === selectedId ? "is-selected" : ""}`}
          onClick={() => onSelect(source.id)}
        >
          <span className="obs-list__meta">
            <strong>{text(source, "display_name")}</strong>
            <em className="mono">{sourceSummary(source)}</em>
          </span>
          <Badge tone={sourceBadgeTone(source)}>{sourceSyncState(source).status}</Badge>
        </button>
      ))}
    </div>
  );
}
