/**
 * The fields each entry type holds. `text` is empty: a text entry is a bare string, not an object.
 */
export const ENTRY_FIELDS: Record<string, readonly string[]> = {
  "text": [],
  "one-line": ["label", "details"],
  "normal": ["name", "location", "date", "start_date", "end_date", "summary"],
  "experience": [
    "company",
    "position",
    "location",
    "date",
    "start_date",
    "end_date",
    "summary",
  ],
  "education": [
    "institution",
    "area",
    "degree",
    "location",
    "date",
    "start_date",
    "end_date",
    "summary",
    "coursework",
  ],
  "publication": ["title", "authors", "doi", "url", "journal", "date", "summary"],
  "bullet": ["bullet"],
  "numbered": ["number"],
  "reversed-numbered": ["reversed_number"],
};

/** Fields holding a list of strings, written one per line. */
export const LIST_FIELDS = ["authors"] as const;

/** The field an entry type keeps as comma-separated elements, editable and reorderable one at a
 * time — the same way a one-line entry's `details` already works. */
export const ELEMENT_FIELDS: Record<string, string> = {
  "one-line": "details",
  "education": "coursework",
};

/** The key a text entry's single value is held under in the form. */
export const TEXT_FIELD = "text";

export function isListField(name: string): boolean {
  return (LIST_FIELDS as readonly string[]).includes(name);
}

/** Builds the fields for one entry type out of the form. Fields left blank are omitted. */
export function buildFields(entryType: string, values: Record<string, string>): unknown {
  if (entryType === "text") {
    return values[TEXT_FIELD] ?? "";
  }

  const fields: Record<string, unknown> = {};
  for (const name of ENTRY_FIELDS[entryType] ?? []) {
    const written = (values[name] ?? "").trim();
    if (!written) continue;
    fields[name] = isListField(name)
      ? written.split("\n").map((line) => line.trim()).filter(Boolean)
      : written;
  }
  return fields;
}

/** Reads an entry's stored fields back into the form. */
export function readFields(entryType: string, fields: unknown): Record<string, string> {
  if (entryType === "text") {
    return { [TEXT_FIELD]: typeof fields === "string" ? fields : "" };
  }

  const held = (fields ?? {}) as Record<string, unknown>;
  const values: Record<string, string> = {};
  for (const name of ENTRY_FIELDS[entryType] ?? []) {
    const value = held[name];
    values[name] = Array.isArray(value)
      ? value.join("\n")
      : value === null || value === undefined
        ? ""
        : String(value);
  }
  return values;
}

/** Entry types whose entries hold accomplishments as Bullets. */
export const BULLET_TYPES = ["normal", "experience", "education"] as const;

export function carriesBullets(entryType: string): boolean {
  return (BULLET_TYPES as readonly string[]).includes(entryType);
}

/** The fields each entry type is titled and subtitled by, in that order. */
const LABEL_FIELDS: Record<string, readonly [string, string | null]> = {
  "one-line": ["label", "details"],
  "normal": ["name", "summary"],
  "experience": ["company", "position"],
  "education": ["institution", "degree"],
  "publication": ["title", "journal"],
  "bullet": ["bullet", null],
  "numbered": ["number", null],
  "reversed-numbered": ["reversed_number", null],
};

/** How an entry names itself in a list: a title, a subtitle, and a date range. */
export function entryLabel(entryType: string, fields: unknown): {
  title: string;
  subtitle: string;
  dates: string;
} {
  if (entryType === "text" || typeof fields !== "object" || fields === null) {
    return { title: String(fields ?? ""), subtitle: "", dates: "" };
  }

  const held = fields as Record<string, unknown>;
  const read = (name: string | null) =>
    name && held[name] !== undefined && held[name] !== null ? String(held[name]) : "";

  const [titleField, subtitleField] = LABEL_FIELDS[entryType] ?? ["name", null];
  const span = [read("start_date"), read("end_date")].filter(Boolean).join("–");

  return {
    title: read(titleField) || "(untitled)",
    subtitle: read(subtitleField),
    dates: read("date") || span,
  };
}
