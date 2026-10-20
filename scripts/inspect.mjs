import { chromium } from "playwright-core";
const b = await chromium.launch({ channel: "chrome", headless: true });
const p = await b.newPage({ viewport: { width: 1280, height: 820 } });
await p.goto("http://localhost:4173/", { waitUntil: "networkidle" });
await p.getByRole("button", { name: "联机", exact: true }).click();
await p.waitForTimeout(500);
const info = await p.evaluate(() => {
  const s = document.querySelector("strong");
  if (!s) return "no strong";
  const cs = getComputedStyle(s);
  const r = s.getBoundingClientRect();
  return { text: s.textContent, font: cs.fontFamily, size: cs.fontSize, lh: cs.lineHeight, weight: cs.fontWeight, pos: cs.position, rect: [r.x, r.y, r.width, r.height] };
});
console.log(JSON.stringify(info));
await b.close();
