import { chromium } from "playwright-core";
const b = await chromium.launch({ channel: "chrome", headless: true });
const p = await b.newPage({ viewport: { width: 1280, height: 820 } });
await p.goto("http://localhost:4173/", { waitUntil: "networkidle" });
await p.locator("aside button", { hasText: "设置" }).first().click();
await p.waitForTimeout(500);
await p.getByRole("button", { name: "浅色" }).click();
await p.waitForTimeout(500);
const r = await p.evaluate(() => {
  const html = document.documentElement;
  const aside = document.querySelector("aside");
  const body = document.body;
  const cs = getComputedStyle(aside);
  return {
    htmlClass: html.className,
    surfaceVar: getComputedStyle(html).getPropertyValue("--surface"),
    asideBg: cs.backgroundColor,
    asideBgImage: cs.backgroundImage,
    bodyBg: getComputedStyle(body).backgroundColor,
    asideClass: aside.className,
  };
});
console.log(JSON.stringify(r, null, 2));
await b.close();
