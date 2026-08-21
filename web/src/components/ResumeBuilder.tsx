import { useEffect, useMemo, useState } from "react";

import { describeEntry } from "@/entryFields";
import { buildResumeYaml, placementKey, type EntryWithBullets } from "@/resumeYaml";
import {
  eligibleEntries,
  listBullets,
  listSections,
  readResume,
  updateVariant,
  writeResume,
  type Section,
} from "@/rpc";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";

/**
 * The two-panel assembly screen (Slice 9, #10): Sections and their eligible Entries on the
 * left, with a checkbox to place each one and a textarea per Bullet's default wording; the
 * resume.yaml that placement would produce, and the file as it stands on disk, on the right.
 */
export function ResumeBuilder() {
  const [sections, setSections] = useState<Section[]>([]);
  const [entriesBySection, setEntriesBySection] = useState<
    Record<string, EntryWithBullets[]>
  >({});
  const [checked, setChecked] = useState<Record<string, boolean>>({});
  const [onDisk, setOnDisk] = useState("");
  const [status, setStatus] = useState("Loading…");

  useEffect(() => {
    void loadAll();
  }, []);

  async function loadAll() {
    try {
      const loadedSections = await listSections();
      const bySection: Record<string, EntryWithBullets[]> = {};
      for (const section of loadedSections) {
        const entries = await eligibleEntries(section.name);
        bySection[section.name] = await Promise.all(
          entries.map(async (entry) => ({ ...entry, bullets: await listBullets(entry.id) })),
        );
      }
      setSections(loadedSections);
      setEntriesBySection(bySection);
      await loadOnDisk();
      setStatus("Loaded.");
    } catch (error) {
      setStatus(`Failed: ${(error as Error).message}`);
    }
  }

  async function loadOnDisk() {
    try {
      setOnDisk(await readResume());
    } catch (error) {
      setOnDisk(`(couldn't read resume.yaml: ${(error as Error).message})`);
    }
  }

  function toggle(sectionName: string, entryId: number, value: boolean) {
    setChecked((previous) => ({ ...previous, [placementKey(sectionName, entryId)]: value }));
  }

  /** Rewords the default Variant of one Bullet, and folds the new text into local state. */
  async function rewordBullet(
    sectionName: string,
    entryId: number,
    bulletId: number,
    variantId: number,
    text: string,
    note: string | null,
  ) {
    try {
      await updateVariant(variantId, text, note);
      setEntriesBySection((previous) => ({
        ...previous,
        [sectionName]: previous[sectionName].map((entry) =>
          entry.id !== entryId
            ? entry
            : {
                ...entry,
                bullets: entry.bullets.map((bullet) =>
                  bullet.id !== bulletId
                    ? bullet
                    : {
                        ...bullet,
                        variants: bullet.variants.map((variant) =>
                          variant.id !== variantId ? variant : { ...variant, text },
                        ),
                      },
                ),
              },
        ),
      }));
      setStatus("Wording saved.");
    } catch (error) {
      setStatus(`Failed: ${(error as Error).message}`);
    }
  }

  const preview = useMemo(
    () => buildResumeYaml(sections, entriesBySection, checked),
    [sections, entriesBySection, checked],
  );

  async function place() {
    try {
      await writeResume(preview);
      setStatus("Wrote resume.yaml.");
      await loadOnDisk();
    } catch (error) {
      setStatus(`Failed: ${(error as Error).message}`);
    }
  }

  return (
    <div className="grid grid-cols-1 gap-6 p-6 lg:grid-cols-2">
      <div className="flex flex-col gap-4">
        <h2 className="font-heading text-lg font-medium">Builder</h2>
        {sections.map((section) => (
          <Card key={section.name}>
            <CardHeader>
              <CardTitle>{section.name}</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              {(entriesBySection[section.name] ?? []).map((entry) => {
                const key = placementKey(section.name, entry.id);
                return (
                  <div
                    key={key}
                    className="flex flex-col gap-2 border-b border-border pb-4 last:border-b-0 last:pb-0"
                  >
                    <label className="flex items-center gap-2">
                      <Checkbox
                        checked={checked[key] ?? false}
                        onCheckedChange={(value) => toggle(section.name, entry.id, value === true)}
                      />
                      <span className="text-sm font-medium">
                        {describeEntry(entry.entry_type, entry.fields)}
                      </span>
                    </label>
                    {entry.bullets.map((bullet) => {
                      const variant = bullet.variants.find((candidate) => candidate.is_default);
                      if (!variant) return null;
                      return (
                        <Textarea
                          key={bullet.id}
                          defaultValue={variant.text}
                          className="ml-6 w-auto"
                          onBlur={(event) =>
                            void rewordBullet(
                              section.name,
                              entry.id,
                              bullet.id,
                              variant.id,
                              event.target.value,
                              variant.note,
                            )
                          }
                        />
                      );
                    })}
                  </div>
                );
              })}
              {(entriesBySection[section.name] ?? []).length === 0 && (
                <p className="text-sm text-muted-foreground">
                  No {section.entry_type} entries yet.
                </p>
              )}
            </CardContent>
          </Card>
        ))}
        {sections.length === 0 && (
          <p className="text-sm text-muted-foreground">No sections yet.</p>
        )}
        <p className="text-sm text-muted-foreground">{status}</p>
      </div>

      <div className="flex flex-col gap-4">
        <h2 className="font-heading text-lg font-medium">Preview</h2>
        <Card>
          <CardHeader>
            <CardTitle>What Place will write</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            <ScrollArea className="h-64 rounded-lg border border-border">
              <pre className="p-3 text-xs">{preview}</pre>
            </ScrollArea>
            <Button onClick={place}>Place into resume.yaml</Button>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>On disk</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            <ScrollArea className="h-64 rounded-lg border border-border">
              <pre className="p-3 text-xs">{onDisk}</pre>
            </ScrollArea>
            <Button variant="outline" onClick={loadOnDisk}>
              Refresh
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
