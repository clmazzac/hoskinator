import { useCallback, useEffect, useState } from "react";
import { ChevronRight } from "lucide-react";

import EditableText from "@/components/EditableText";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { getProfile, setProfile, type Profile } from "@/rpc";

/// Networks rendercv can render a connection for, spelled as it spells them.
const NETWORKS = [
  "LinkedIn",
  "GitHub",
  "GitLab",
  "IMDB",
  "Instagram",
  "ORCID",
  "Mastodon",
  "StackOverflow",
  "ResearchGate",
  "YouTube",
  "Google Scholar",
  "Telegram",
  "WhatsApp",
  "Leetcode",
  "X",
  "Bluesky",
  "Reddit",
] as const;

/// Fields holding one value or several. The stored shape is preserved: a list of one stays a
/// list, because collapsing it to a scalar would change the data behind a form control.
const ONE_OR_MANY = ["email", "phone", "website"] as const;

const PLAIN = ["name", "headline", "location", "photo"] as const;

function read(value: unknown): string {
  if (Array.isArray(value)) return value.join("\n");
  return value == null ? "" : String(value);
}

function write(previous: unknown, written: string): unknown {
  const lines = written
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length === 0) return null;
  if (lines.length > 1) return lines;
  return Array.isArray(previous) ? lines : lines[0];
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline gap-2 py-0.5">
      <span className="w-20 shrink-0 text-right text-[10px] text-muted-foreground">
        {label}
      </span>
      {children}
    </div>
  );
}

export default function ProfileNode() {
  const [open, setOpen] = useState(false);
  const [profile, setLoaded] = useState<Profile | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getProfile().then(setLoaded, (failure: Error) => setError(failure.message));
  }, []);

  useEffect(load, [load]);

  const commit = (next: Profile) => {
    setLoaded(next);
    setProfile(next).then(load, (failure: Error) => {
      setError(failure.message);
      load();
    });
  };

  const networks = profile?.social_networks ?? [];

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div className="flex items-center gap-1 border-b px-2 py-1.5 hover:bg-muted/40">
        <span className="size-4 shrink-0" />
        <CollapsibleTrigger className="grid size-3.5 shrink-0 place-items-center">
          <ChevronRight
            className={cn(
              "size-3.5 text-muted-foreground transition-transform",
              open && "rotate-90",
            )}
          />
        </CollapsibleTrigger>
        <span className="text-xs font-semibold">Profile</span>
        <span className="flex-1" />
        <span className="shrink-0 text-[10px] text-muted-foreground">
          {profile?.name ?? ""}
        </span>
      </div>

      <CollapsibleContent className="border-b">
        {error && <p className="px-3 py-1.5 text-xs text-destructive">{error}</p>}
        {profile && (
          <div className="grid gap-0.5 py-1 pr-2 pl-7">
            {PLAIN.map((field) => (
              <Row key={field} label={field}>
                <EditableText
                  value={read(profile[field])}
                  onCommit={(next) =>
                    commit({ ...profile, [field]: next.trim() || null })
                  }
                  placeholder="—"
                  className="flex-1 text-xs"
                />
              </Row>
            ))}

            {ONE_OR_MANY.map((field) => (
              <Row key={field} label={field}>
                <EditableText
                  value={read(profile[field])}
                  onCommit={(next) =>
                    commit({ ...profile, [field]: write(profile[field], next) })
                  }
                  placeholder="—"
                  className="flex-1 text-xs"
                  multiline={Array.isArray(profile[field])}
                />
              </Row>
            ))}

            {networks.map((connection, index) => (
              <Row key={index} label={index === 0 ? "connections" : ""}>
                <Select
                  value={connection.network}
                  onValueChange={(network) => {
                    if (!network) return;
                    const next = networks.map((held, at) =>
                      at === index ? { ...held, network } : held,
                    );
                    commit({ ...profile, social_networks: next });
                  }}
                >
                  <SelectTrigger
                    size="sm"
                    className="h-6 w-32 border-transparent text-xs shadow-none hover:border-border"
                    aria-label="Network"
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {NETWORKS.map((name) => (
                      <SelectItem key={name} value={name} className="text-xs">
                        {name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <EditableText
                  value={connection.username}
                  onCommit={(username) => {
                    const next = networks
                      .map((held, at) =>
                        at === index ? { ...held, username: username.trim() } : held,
                      )
                      .filter((held) => held.username !== "");
                    commit({ ...profile, social_networks: next });
                  }}
                  placeholder="(empty — commit to remove)"
                  className="flex-1 text-xs"
                />
              </Row>
            ))}

            <Row label={networks.length === 0 ? "connections" : ""}>
              <EditableText
                value=""
                onCommit={(username) => {
                  if (!username.trim()) return;
                  commit({
                    ...profile,
                    social_networks: [
                      ...networks,
                      { network: "GitHub", username: username.trim() },
                    ],
                  });
                }}
                placeholder="add a username…"
                className="flex-1 text-xs"
              />
            </Row>
          </div>
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}
