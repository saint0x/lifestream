import { useEffect, useMemo, useState } from "react";
import { Copy, Eye, EyeOff, KeyRound, Plus, Trash2 } from "lucide-react";
import { requestJson } from "@/lib/api";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Badge } from "@/components/ui/Badge";
import { PageTrail } from "@/components/navigation/PageTrail";
import type { CreatorApiKey, CreatorApiKeyTokenResponse } from "@/types";
import "./DeveloperPage.css";

const defaultScopes = [
  "creator:read",
  "creator:profile:write",
  "creator:uploads:read",
  "creator:uploads:write",
  "creator:live:read",
  "creator:live:control",
] as const;

function maskToken(token: string): string {
  if (token.length <= 18) return "••••••••••••";
  return `${token.slice(0, 14)}••••••••••••${token.slice(-6)}`;
}

function dateLabel(value?: string | null): string {
  if (!value) return "Never";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  }).format(new Date(value));
}

export function DeveloperPage() {
  const [keys, setKeys] = useState<ReadonlyArray<CreatorApiKey>>([]);
  const [name, setName] = useState("Studio API");
  const [visible, setVisible] = useState<ReadonlySet<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const activeKeys = useMemo(
    () => keys.filter((key) => key.revokedAt == null),
    [keys],
  );

  useEffect(() => {
    let alive = true;
    requestJson<ReadonlyArray<CreatorApiKey>>("/api/v1/me/api-keys")
      .then((items) => {
        if (alive) setKeys(items);
      })
      .catch((error) => {
        if (alive) setStatus(error instanceof Error ? error.message : "Unable to load API keys.");
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, []);

  const createKey = async () => {
    setSaving(true);
    setStatus(null);
    try {
      const response = await requestJson<CreatorApiKeyTokenResponse>("/api/v1/me/api-keys", {
        method: "POST",
        body: {
          name: name.trim() || "Studio API",
          scopes: [...defaultScopes],
        },
      });
      setKeys((current) => [response.apiKey, ...current]);
      setVisible((current) => new Set(current).add(response.apiKey.id));
      setStatus("API key created.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Unable to create API key.");
    } finally {
      setSaving(false);
    }
  };

  const revokeKey = async (id: string) => {
    setStatus(null);
    await requestJson<void>(`/api/v1/me/api-keys/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
    setKeys((current) => current.filter((key) => key.id !== id));
    setStatus("API key revoked.");
  };

  const toggleVisible = (id: string) => {
    setVisible((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const copyToken = async (token: string) => {
    await navigator.clipboard.writeText(token);
    setStatus("API key copied.");
  };

  return (
    <div className="ls-dev">
      <header className="ls-dev__head">
        <PageTrail
          className="ls-dev__kicker mono"
          items={[
            { label: "Dashboard", href: "/" },
            { label: "Developer" },
          ]}
        />
        <h1 className="ls-dev__title">Developer</h1>
        <p className="ls-dev__sub">
          Manage Creator Studio with a scoped API key. Playback and viewer attention are not exposed.
        </p>
      </header>

      <section className="ls-dev__panel">
        <div className="ls-dev__panel-head">
          <div>
            <div className="ls-dev__eyebrow mono">API Keys</div>
            <h2>Create a key</h2>
          </div>
          <Badge tone="premium">Creator API</Badge>
        </div>
        <div className="ls-dev__create">
          <Input
            icon={<KeyRound size={16} />}
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="Key name"
          />
          <Button icon={<Plus />} onClick={() => void createKey()} disabled={saving}>
            Create key
          </Button>
        </div>
        <div className="ls-dev__base mono">
          <span>Base URL</span>
          <code>https://api-production-4becb.up.railway.app/api/v1/creator-api</code>
        </div>
      </section>

      <section className="ls-dev__panel">
        <div className="ls-dev__panel-head">
          <div>
            <div className="ls-dev__eyebrow mono">Active Keys</div>
            <h2>{loading ? "Loading" : `${activeKeys.length} keys`}</h2>
          </div>
        </div>
        {activeKeys.length === 0 && !loading ? (
          <div className="ls-dev__empty">No active API keys yet.</div>
        ) : (
          <div className="ls-dev__keys">
            {activeKeys.map((key) => {
              const isVisible = visible.has(key.id);
              return (
                <article className="ls-dev__key" key={key.id}>
                  <div className="ls-dev__key-main">
                    <div>
                      <h3>{key.name}</h3>
                      <p className="mono">Created {dateLabel(key.createdAt)} / Used {dateLabel(key.lastUsedAt)}</p>
                    </div>
                    <div className="ls-dev__key-actions">
                      <Button
                        variant="ghost"
                        icon={isVisible ? <EyeOff /> : <Eye />}
                        aria-label={isVisible ? "Hide API key" : "Show API key"}
                        onClick={() => toggleVisible(key.id)}
                      />
                      <Button
                        variant="ghost"
                        icon={<Copy />}
                        aria-label="Copy API key"
                        onClick={() => void copyToken(key.accessToken)}
                      />
                      <Button
                        variant="ghost"
                        icon={<Trash2 />}
                        aria-label="Revoke API key"
                        onClick={() => void revokeKey(key.id)}
                      />
                    </div>
                  </div>
                  <code className="ls-dev__token">
                    {isVisible ? key.accessToken : maskToken(key.accessToken)}
                  </code>
                  <div className="ls-dev__scopes">
                    {key.scopes.map((scope) => (
                      <span key={scope}>{scope}</span>
                    ))}
                  </div>
                </article>
              );
            })}
          </div>
        )}
        {status ? <div className="ls-dev__status">{status}</div> : null}
      </section>
    </div>
  );
}
