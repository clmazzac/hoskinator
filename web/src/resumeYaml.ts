import { stringify } from "yaml";

import type { Bullet, Entry, Section } from "./rpc";

/** An Entry with the Bullets it carries, each already loaded with its Variants. */
export type EntryWithBullets = Entry & { bullets: Bullet[] };

function defaultVariantText(bullet: Bullet): string | null {
  return bullet.variants.find((variant) => variant.is_default)?.text ?? null;
}

/** Renders one Entry into the shape rendercv expects under `cv.sections.<name>`. */
function renderEntry(entryType: string, entry: EntryWithBullets): unknown {
  if (entryType === "text") {
    return typeof entry.fields === "string" ? entry.fields : "";
  }

  const highlights = entry.bullets
    .map(defaultVariantText)
    .filter((text): text is string => text !== null);
  const fields = (entry.fields ?? {}) as Record<string, unknown>;
  return highlights.length > 0 ? { ...fields, highlights } : { ...fields };
}

/** The key `checked` holds placement under: one Entry can be eligible for several Sections. */
export function placementKey(sectionName: string, entryId: number): string {
  return `${sectionName}::${entryId}`;
}

/**
 * Assembles the `cv.sections` YAML that `resume.write` should receive, from whichever
 * (section, entry) pairs are checked. Placement here is a snapshot of the builder's local
 * state, not a diff against the file on disk — matching Slice 9's no-reconciliation rule
 * (docs/agents / issue #10): placing is explicit, and nothing here infers what was placed before.
 */
export function buildResumeYaml(
  sections: Section[],
  entriesBySection: Record<string, EntryWithBullets[]>,
  checked: Record<string, boolean>,
): string {
  const cvSections: Record<string, unknown[]> = {};
  for (const section of sections) {
    const entries = entriesBySection[section.name] ?? [];
    const placed = entries
      .filter((entry) => checked[placementKey(section.name, entry.id)])
      .map((entry) => renderEntry(section.entry_type, entry));
    if (placed.length > 0) {
      cvSections[section.name] = placed;
    }
  }
  return stringify({ cv: { sections: cvSections } });
}
