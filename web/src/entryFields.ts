/**
 * The fields each entry type holds. `text` is empty: a text entry is a bare string, not an object.
 */
export const ENTRY_FIELDS: Record<string, readonly string[]> = {
  "text": [],
  "one-line": ["label", "details"],
  "normal": ["name", "location", "date", "start_date", "end_date", "summary", "highlights"],
  "experience": [
    "company",
    "position",
    "location",
    "date",
    "start_date",
    "end_date",
    "summary",
    "highlights",
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
    "highlights",
  ],
  "publication": ["title", "authors", "doi", "url", "journal", "date", "summary"],
  "bullet": ["bullet"],
  "numbered": ["number"],
  "reversed-numbered": ["reversed_number"],
};

/** Fields holding a list of strings, written one per line. */
export const LIST_FIELDS = ["highlights", "authors"] as const;

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
