// Moving the tracker in and out of a spreadsheet.
//
// Google Sheets copies to the clipboard as tab-separated rows and exports as CSV, so the importer
// accepts both. Columns are matched by their heading, which is what a user's own sheet actually
// carries — the screenshot this was built from names them Company, Position, Date Applied,
// Application Status, Listing Page, Resume, and Notes.

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
};

const STATUSES = ["draft", "applied", "interview", "offer", "rejected"];

/// Splits one delimited line, honouring quoted fields.
function cells(line: string, delimiter: string): string[] {
  const out: string[] = [];
  let held = "";
  let quoted = false;

  for (let at = 0; at < line.length; at += 1) {
    const character = line[at];
    if (quoted) {
      if (character === '"' && line[at + 1] === '"') {
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
      out.push(held.trim());
      held = "";
    } else {
      held += character;
    }
  }
  out.push(held.trim());
  return out;
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

/// Reads pasted rows into applications. The first line must be the headings.
export function parseSheet(text: string): NewApplication[] {
  const lines = text.split(/\r?\n/).filter((line) => line.trim() !== "");
  if (lines.length < 2) return [];

  const delimiter = delimiterOf(text);
  const headings = cells(lines[0], delimiter).map((heading) => heading.toLowerCase());

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
  };

  const read = (row: string[], index: number) =>
    index >= 0 && index < row.length ? row[index] : "";

  return lines
    .slice(1)
    .map((line) => cells(line, delimiter))
    .filter((row) => read(row, at.company) !== "" || read(row, at.position) !== "")
    .map((row) => ({
      company: read(row, at.company),
      position: read(row, at.position),
      status: readStatus(read(row, at.status)),
      date_applied: readDate(read(row, at.date_applied)),
      listing_url: read(row, at.listing_url) || null,
      resume_branch: read(row, at.resume_branch) || null,
      notes: read(row, at.notes) || null,
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
    ]
      .map(quote)
      .join(","),
  );

  return [
    "Company,Position,Date Applied,Application Status,Listing Page,Resume,Notes",
    ...rows,
  ].join("\n");
}
