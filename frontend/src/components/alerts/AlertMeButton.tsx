import { useMemo, useState } from "react";
import { Bell, Check, MessageCircle, Smartphone, X } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { getVisitorId } from "@/lib/attribution";
import { repository } from "@/lib/repository";
import type { PublicAlertContactChannel, PublicAlertTargetKind } from "@/types";
import "./AlertMeButton.css";

interface AlertMeButtonProps {
  readonly targetKind: PublicAlertTargetKind;
  readonly targetId: string;
  readonly targetSlug?: string | null;
  readonly targetTitle: string;
  readonly alertTypes: ReadonlyArray<string>;
  readonly label?: string;
}

const methodOptions: ReadonlyArray<{
  readonly key: PublicAlertContactChannel;
  readonly label: string;
  readonly placeholder: string;
}> = [
  { key: "email", label: "Email", placeholder: "you@example.com" },
  { key: "sms", label: "Text", placeholder: "+1 555 555 0199" },
  { key: "social_dm", label: "DM", placeholder: "@handle or profile URL" },
];

const socialPlatforms = ["instagram", "x", "tiktok", "facebook", "linkedin"];

function defaultPlaceholder(channel: PublicAlertContactChannel): string {
  return methodOptions.find((item) => item.key === channel)?.placeholder ?? "Contact";
}

export function AlertMeButton({
  targetKind,
  targetId,
  targetSlug,
  targetTitle,
  alertTypes,
  label = "Alert me",
}: AlertMeButtonProps) {
  const [open, setOpen] = useState(false);
  const [channel, setChannel] = useState<PublicAlertContactChannel>("email");
  const [contact, setContact] = useState("");
  const [socialPlatform, setSocialPlatform] = useState("instagram");
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const title = useMemo(() => {
    if (targetKind === "profile") return `Alerts from ${targetTitle}`;
    if (targetKind === "episode") return `Alerts for ${targetTitle}`;
    return `Alerts for ${targetTitle}`;
  }, [targetKind, targetTitle]);

  const submit = async () => {
    const contactValue = contact.trim();
    if (!contactValue) {
      setError("Add where we should send the alert.");
      return;
    }
    setSaving(true);
    setError(null);
    setStatus(null);
    try {
      await repository.createPublicAlertSubscription({
        targetKind,
        targetId,
        targetSlug,
        targetTitle,
        visitorId: getVisitorId(),
        contactChannel: channel,
        contactValue,
        socialPlatform: channel === "social_dm" ? socialPlatform : null,
        alertTypes,
        sourcePath: window.location.pathname,
      });
      setStatus("Alert saved.");
      setContact("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to save this alert.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="ls-alert">
      <Button
        variant="outline"
        size="sm"
        icon={status ? <Check /> : <Bell />}
        onClick={() => {
          setOpen((current) => !current);
          setError(null);
        }}
      >
        {status ?? label}
      </Button>
      {open ? (
        <div className="ls-alert__popover" role="dialog" aria-label={title}>
          <div className="ls-alert__head">
            <div>
              <div className="ls-alert__kicker mono">release alerts</div>
              <strong>{title}</strong>
            </div>
            <button type="button" className="ls-alert__close" aria-label="Close alert form" onClick={() => setOpen(false)}>
              <X size={14} />
            </button>
          </div>
          <div className="ls-alert__methods" role="tablist" aria-label="Alert method">
            {methodOptions.map((item) => (
              <button
                key={item.key}
                type="button"
                className={channel === item.key ? "is-active" : ""}
                onClick={() => {
                  setChannel(item.key);
                  setError(null);
                }}
              >
                {item.key === "email" ? <MessageCircle size={13} /> : null}
                {item.key === "sms" ? <Smartphone size={13} /> : null}
                {item.key === "social_dm" ? <Bell size={13} /> : null}
                {item.label}
              </button>
            ))}
          </div>
          {channel === "social_dm" ? (
            <select
              className="ls-alert__select"
              value={socialPlatform}
              onChange={(event) => setSocialPlatform(event.target.value)}
            >
              {socialPlatforms.map((platform) => (
                <option key={platform} value={platform}>{platform}</option>
              ))}
            </select>
          ) : null}
          <Input
            value={contact}
            onChange={(event) => setContact(event.target.value)}
            placeholder={defaultPlaceholder(channel)}
          />
          {error ? <div className="ls-alert__message is-error">{error}</div> : null}
          {status ? <div className="ls-alert__message">{status}</div> : null}
          <Button variant="primary" size="sm" icon={<Bell />} onClick={() => void submit()} disabled={saving}>
            {saving ? "Saving..." : "Save alert"}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
