/// Every drag the two outline columns offer, checked against what reaches resume.yaml.
///
/// The daemon must be running, and the store it serves must hold the sections these tests name.

import { expect, test, type Page } from "@playwright/test";

import {
  bankFieldGrip,
  bankGrip,
  openBankRow,
  resumeChip,
  resumeChips,
  resumeText,
  settled,
} from "./editor";
import {
  bullets,
  details,
  eligible,
  entryTitles,
  outline,
  readResume,
  section,
  writeResume,
  type StoreEntry,
} from "./rpc";

/// The document every test starts from: two experience entries and one one-line entry.
const FIXTURE = `cv:
  name: Barnaby Q. Fenwhistle
  sections:
    Experience:
      - company: Helio Systems
        position: Staff Software Engineer
        start_date: 2022-06
        end_date: present
        highlights:
          - FIRST WORDING
          - SECOND WORDING
      - company: Ravensmoor Analytics
        position: Backend Engineer
        start_date: 2019-08
        end_date: 2022-05
        highlights:
          - RAVENSMOOR WORDING
    Skills:
      - label: Languages
        details: Rust, Go, Python
design:
  theme: classic
`;

const elementsOf = (entry: StoreEntry) =>
  String(entry.fields?.details ?? "")
    .split(",")
    .map((element) => element.trim())
    .filter(Boolean);

let held: string;
let bankExperience: StoreEntry[];
let bankSkills: StoreEntry[];
let single: string;
let multiple: { shown: string; other: string };
let element: string;

test.beforeAll(async () => {
  held = await readResume();
  bankExperience = await eligible("Experience");
  bankSkills = await eligible("Skills");
  expect(bankExperience.length, "the store needs three Experience entries").toBeGreaterThan(2);
  expect(bankSkills.length, "the store needs two Skills entries").toBeGreaterThan(1);

  const accomplishments = await bullets(bankExperience[0].id);
  const alone = accomplishments.find((bullet) => bullet.variants.length === 1);
  const worded = accomplishments.find((bullet) => bullet.variants.length > 1);
  expect(alone, "the store needs a bullet with one wording").toBeTruthy();
  expect(worded, "the store needs a bullet with several wordings").toBeTruthy();
  single = alone!.variants[0].text;
  multiple = {
    shown: (worded!.variants.find((variant) => variant.is_default) ?? worded!.variants[0]).text,
    other: worded!.variants.filter((variant) => !variant.is_default)[0].text,
  };

  element = elementsOf(bankSkills[1])[2];
  expect(element, "the store needs a Skills entry with three elements").toBeTruthy();
});

test.afterAll(async () => {
  await writeResume(held);
});

test.beforeEach(async ({ page }) => {
  await writeResume(FIXTURE);
  await page.goto("/");
  await expect(resumeText(page, "Helio Systems")).toBeVisible();
});

/// Opens the bank down to the bullets of the first Experience entry.
async function openFirstExperienceEntry(page: Page): Promise<void> {
  await openBankRow(page, "Experience");
  await openBankRow(page, String(bankExperience[0].fields?.company));
}

/// Opens the bank down to the elements of the second Skills entry.
async function openSecondSkillsEntry(page: Page): Promise<void> {
  await openBankRow(page, "Skills");
  await openBankRow(page, String(bankSkills[1].fields?.label));
}

const highlights = async (index: number) =>
  (await section("Experience")).entries[index].highlights;

test("a bullet with one wording drops into an entry", async ({ page }) => {
  await openFirstExperienceEntry(page);
  const grip = await bankFieldGrip(page, single);

  await grip.dragTo(resumeText(page, "Ravensmoor Analytics"));

  await expect.poll(() => highlights(1)).toEqual(["RAVENSMOOR WORDING", single]);
});

test("a bullet with several wordings drops the wording that was dragged", async ({ page }) => {
  await openFirstExperienceEntry(page);
  const shown = await bankFieldGrip(page, multiple.shown);
  await shown.locator("xpath=following-sibling::button").first().click();
  const other = await bankFieldGrip(page, multiple.other);

  await other.dragTo(resumeText(page, "Ravensmoor Analytics"));

  await expect.poll(() => highlights(1)).toEqual(["RAVENSMOOR WORDING", multiple.other]);
});

test("an entry drops onto a section header", async ({ page }) => {
  await openBankRow(page, "Experience");
  const label = String(bankExperience[2].fields?.company);

  await bankGrip(page, label).dragTo(resumeText(page, "Experience"));

  await expect
    .poll(() => entryTitles("Experience"))
    .toEqual(["Helio Systems", "Ravensmoor Analytics", label]);
});

test("an entry drops onto another entry", async ({ page }) => {
  await openBankRow(page, "Experience");
  const label = String(bankExperience[2].fields?.company);

  await bankGrip(page, label).dragTo(resumeText(page, "Helio Systems"));

  await expect
    .poll(() => entryTitles("Experience"))
    .toEqual(["Helio Systems", "Ravensmoor Analytics", label]);
});

test("an entry of the wrong type does not drop into a section", async ({ page }) => {
  await openBankRow(page, "Skills");
  const label = String(bankSkills[0].fields?.label);

  await bankGrip(page, label).dragTo(resumeText(page, "Experience"));

  await settled(page);
  expect(await entryTitles("Experience")).toEqual(["Helio Systems", "Ravensmoor Analytics"]);
});

test("a section drops into the resume", async ({ page }) => {
  await bankGrip(page, "Publications").dragTo(resumeText(page, "Experience"));

  await expect
    .poll(async () => (await outline()).map((held) => held.name))
    .toEqual(["Experience", "Skills", "Publications"]);
});

test("an entry reorders onto the entry above it", async ({ page }) => {
  await resumeText(page, "Ravensmoor Analytics").dragTo(resumeText(page, "Helio Systems"));

  await expect
    .poll(() => entryTitles("Experience"))
    .toEqual(["Ravensmoor Analytics", "Helio Systems"]);
});

test("a wording reorders inside its entry", async ({ page }) => {
  await resumeText(page, "SECOND WORDING").dragTo(resumeText(page, "FIRST WORDING"));

  await expect.poll(() => highlights(0)).toEqual(["SECOND WORDING", "FIRST WORDING"]);
});

test("an element drops onto a one-line entry", async ({ page }) => {
  await openSecondSkillsEntry(page);
  const grip = await bankFieldGrip(page, element);

  await grip.dragTo(resumeText(page, "Languages"));

  await expect.poll(() => details("Skills", 0)).toBe(`Rust, Go, Python, ${element}`);
});

test("an element dropped onto a chip is added, not read as a reorder", async ({ page }) => {
  await openSecondSkillsEntry(page);
  const grip = await bankFieldGrip(page, element);

  await grip.dragTo(resumeChip(page, 2));

  await expect.poll(() => details("Skills", 0)).toBe(`Rust, Go, Python, ${element}`);
});

test("an entry dropped onto a chip still reaches the section", async ({ page }) => {
  await openBankRow(page, "Skills");
  const label = String(bankSkills[1].fields?.label);

  await bankGrip(page, label).dragTo(resumeChip(page, 1));

  await expect.poll(() => entryTitles("Skills")).toEqual(["Languages", label]);
});

test("chips reorder inside their entry", async ({ page }) => {
  await resumeChip(page, 2).dragTo(resumeChip(page, 0));

  await expect.poll(() => details("Skills", 0)).toBe("Python, Rust, Go");
});

test("a chip dropped clear of every chip is not copied back in", async ({ page }) => {
  await expect(resumeChips(page)).toHaveCount(3);

  await resumeChip(page, 2).dragTo(resumeText(page, "Languages"));

  await settled(page);
  expect(await details("Skills", 0)).toBe("Rust, Go, Python");
});

test("a wording dropped clear of the wording list reorders nothing", async ({ page }) => {
  // Onto the header of the entry it belongs to, then onto a different entry: a wording only
  // reorders against another wording of its own entry, so neither drop may reach the entries.
  await resumeText(page, "SECOND WORDING").dragTo(resumeText(page, "Helio Systems"));
  await settled(page);
  await resumeText(page, "SECOND WORDING").dragTo(resumeText(page, "Ravensmoor Analytics"));
  await settled(page);

  expect(await entryTitles("Experience")).toEqual(["Helio Systems", "Ravensmoor Analytics"]);
  expect(await highlights(0)).toEqual(["FIRST WORDING", "SECOND WORDING"]);
  expect(await highlights(1)).toEqual(["RAVENSMOOR WORDING"]);
});

test("an entry dropped onto another entry's lower half lands after it", async ({ page }) => {
  // The pointer rests on RAVENSMOOR WORDING, in the lower half of the Ravensmoor
  // entry, so Helio Systems moves to the far end rather than before its target.
  await resumeText(page, "Helio Systems").dragTo(resumeText(page, "RAVENSMOOR WORDING"));

  await expect
    .poll(() => entryTitles("Experience"))
    .toEqual(["Ravensmoor Analytics", "Helio Systems"]);
  await expect
    .poll(async () => (await section("Experience")).entries[0].highlights)
    .toEqual(["RAVENSMOOR WORDING"]);
});
