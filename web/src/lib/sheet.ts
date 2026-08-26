// Moving the tracker in and out of a spreadsheet.
//
// Google Sheets copies to the clipboard as tab-separated rows and exports as CSV, so the importer
// accepts both. Columns are matched by their heading.

import type { NewApplication } from "@/rpc";

/// Headings recognised for each field, lowercased.
const COLUMNS: Record<keyof NewApplication, string[]> = {
  company: ["company", "employer", "organisation", "organization"],
  position: ["position", "role", "title", "job title"],
  status: ["application status", "status"],
  date_applied: ["date applied", "applied", "date"],
  listing_url: ["listing page", "listing", "url", "link", "posting"],
  resume_branch: ["resume branch", "branch", "resume"],
  notes: ["notes", "note", "comments"],
  jd_text: ["job description", "description", "posting", "jd"],
};

const STATUSES = ["draft", "applied", "interview", "offer", "rejected"];

/// Splits delimited text into rows of cells, honouring quotes — including a delimiter or a
/// literal newline inside one, which a cell exported from Sheets can carry.
function rows(text: string, delimiter: string): string[][] {
  const table: string[][] = [];
  let row: string[] = [];
  let held = "";
  let quoted = false;

  const endCell = () => {
    row.push(held.trim());
    held = "";
  };
  const endRow = () => {
    endCell();
    table.push(row);
    row = [];
  };

  for (let at = 0; at < text.length; at += 1) {
    const character = text[at];
    if (quoted) {
      if (character === '"' && text[at + 1] === '"') {
        held += '"';
        at += 1;
      } else if (character === '"') {
        quoted = false;
      } else {
        held += character;
      }
    } else if (character === '"') {
      quoted = true;
    } else if (character === delimiter) {
      endCell();
    } else if (character === "\r") {
      // Skipped; "\n" (or the loop ending) closes the row.
    } else if (character === "\n") {
      endRow();
    } else {
      held += character;
    }
  }
  if (held !== "" || row.length > 0) endRow();

  return table.filter((cells) => cells.some((cell) => cell !== ""));
}

function delimiterOf(text: string): string {
  const first = text.split("\n")[0] ?? "";
  return first.includes("\t") ? "\t" : ",";
}

/// Reads a date into the ISO form the store holds, leaving anything unrecognised alone.
function readDate(written: string): string | null {
  if (!written) return null;
  const slashed = written.match(/^(\d{1,2})\/(\d{1,2})\/(\d{4})$/);
  if (slashed) {
    const [, month, day, year] = slashed;
    return `${year}-${month.padStart(2, "0")}-${day.padStart(2, "0")}`;
  }
  return written;
}

function readStatus(written: string): string {
  const lowered = written.trim().toLowerCase();
  return STATUSES.find((status) => lowered.startsWith(status)) ?? "draft";
}

/// How many fields a candidate heading row's cells match, lowercased.
function headingScore(cells: string[]): number {
  const headings = cells.map((heading) => heading.toLowerCase());
  return (Object.keys(COLUMNS) as (keyof NewApplication)[]).filter((field) =>
    headings.some((heading) =>
      COLUMNS[field].some((candidate) => heading === candidate || heading.includes(candidate)),
    ),
  ).length;
}

/// Finds the heading row: the one whose cells best match known headings. A synced sheet often
/// carries a title or a summary block above its real table, which this skips past.
function findHeadingRow(table: string[][]): number {
  let best = { at: 0, score: 0 };
  for (let at = 0; at < table.length; at += 1) {
    const score = headingScore(table[at]);
    if (score > best.score) best = { at, score };
  }
  return best.score >= 3 ? best.at : 0;
}

/// Reads rows into applications, from wherever the headings sit — the first row by default, or
/// further down if it looks like a synced sheet's title or summary block sits above them.
export function parseSheet(text: string): NewApplication[] {
  const table = rows(text, delimiterOf(text));
  if (table.length < 2) return [];

  const headingRow = findHeadingRow(table);
  const headings = table[headingRow].map((heading) => heading.toLowerCase());

  const columnFor = (field: keyof NewApplication) =>
    headings.findIndex((heading) =>
      COLUMNS[field].some((candidate) => heading === candidate || heading.includes(candidate)),
    );

  const at = {
    company: columnFor("company"),
    position: columnFor("position"),
    status: columnFor("status"),
    date_applied: columnFor("date_applied"),
    listing_url: columnFor("listing_url"),
    resume_branch: columnFor("resume_branch"),
    notes: columnFor("notes"),
    jd_text: columnFor("jd_text"),
  };

  const read = (row: string[], index: number) =>
    index >= 0 && index < row.length ? row[index] : "";

  return table
    .slice(headingRow + 1)
    .filter((row) => read(row, at.company) !== "" || read(row, at.position) !== "")
    .map((row) => ({
      company: read(row, at.company),
      position: read(row, at.position),
      status: readStatus(read(row, at.status)),
      date_applied: readDate(read(row, at.date_applied)),
      listing_url: read(row, at.listing_url) || null,
      resume_branch: read(row, at.resume_branch) || null,
      notes: read(row, at.notes) || null,
      jd_text: read(row, at.jd_text) || null,
    }));
}

/// Writes applications back out as CSV, headed the way a sheet expects.
export function toCsv(applications: NewApplication[]): string {
  const quote = (value: string | null) => {
    const written = value ?? "";
    return /[",\n]/.test(written) ? `"${written.replace(/"/g, '""')}"` : written;
  };

  const rows = applications.map((one) =>
    [
      one.company,
      one.position,
      one.date_applied,
      one.status,
      one.listing_url,
      one.resume_branch,
      one.notes,
      one.jd_text,
    ]
      .map(quote)
      .join(","),
  );

  return [
    "Company,Position,Date Applied,Application Status,Listing Page,Resume,Notes,Job Description",
    ...rows,
  ].join("\n");
}
