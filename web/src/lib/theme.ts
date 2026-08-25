// The theme is a `dark` class on the document element. `index.html` applies the
// saved value before first paint; this module is how the app reads and changes it.

const STORAGE_KEY = "hoskinator-theme";

export function isDark(): boolean {
  if (typeof document === "undefined") {
    return false;
  }
  return document.documentElement.classList.contains("dark");
}

export function setDark(dark: boolean): void {
  document.documentElement.classList.toggle("dark", dark);
  localStorage.setItem(STORAGE_KEY, dark ? "dark" : "light");
}
