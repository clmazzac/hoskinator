/// Reaching the drag handles and drop targets of the two outline columns.

import { expect, type Locator, type Page } from "@playwright/test";

const GRIP = 'span[title="Drag into the resume"]';

/// The grip of the row a field belongs to, however deeply the field is nested in it.
const OWN_GRIP = `xpath=ancestor::div[./span[@title="Drag into the resume"]][1]/span[@title="Drag into the resume"]`;

export const store = (page: Page) => page.locator("#store");
export const resume = (page: Page) => page.locator("#resume");

/// Opens a bank row by the label it shows, and waits for what it holds to arrive.
export async function openBankRow(page: Page, label: string): Promise<void> {
  const row = store(page).locator(`span:text-is("${label}")`).first().locator("xpath=..");
  const before = await store(page).locator(GRIP).count();
  await row.locator("button").first().click();
  await expect
    .poll(() => store(page).locator(GRIP).count())
    .toBeGreaterThan(before);
}

/// The grip of the bank row titled `label`.
export function bankGrip(page: Page, label: string): Locator {
  return store(page).locator(`span:text-is("${label}")`).first().locator(OWN_GRIP);
}

/// The grip of the bank row whose editable field holds `value`.
///
/// Wordings and elements are rendered into inputs, which carry no text to match on.
export async function bankFieldGrip(page: Page, value: string): Promise<Locator> {
  const fields = store(page).locator("input, textarea");
  for (let at = 0; at < (await fields.count()); at += 1) {
    if ((await fields.nth(at).inputValue()) === value) return fields.nth(at).locator(OWN_GRIP);
  }
  throw new Error(`no bank field holds ${JSON.stringify(value)}`);
}

/// A resume row addressed by the text it shows: an entry title, a wording, or a section name.
export function resumeText(page: Page, text: string): Locator {
  return resume(page).locator(`span:text-is("${text}")`).first();
}

/// One element chip of a one-line entry, counted across the whole resume column.
export function resumeChip(page: Page, index: number): Locator {
  return resume(page).locator("span.cursor-grab").nth(index);
}

export function resumeChips(page: Page): Locator {
  return resume(page).locator("span.cursor-grab");
}

/// Waits out whatever a drop set going, so a test can assert that nothing happened.
export async function settled(page: Page): Promise<void> {
  await page.waitForLoadState("networkidle");
}
