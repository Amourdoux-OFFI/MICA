import { chromium } from "playwright-core";
const OUT = "D:\\Desktop\\DSH\\MICA\\.data\\shots";
const b = await chromium.launch({ channel: "chrome", headless: true });
const p = await b.newPage({ viewport: { width: 1280, height: 820 } });
p.on("pageerror", e => console.log("[pageerror]", e.message));
await p.goto("http://localhost:4173/", { waitUntil: "networkidle" });
await p.waitForTimeout(800);
await p.getByRole("button", { name: "实例", exact: true }).click();
await p.waitForTimeout(700);
const card = p.locator("button.card").first();
if (await card.count()) { await card.click(); await p.waitForTimeout(700); }
const tab = p.locator("button.tab", { hasText: "汉化" });
console.log("tab count:", await tab.count());
if (await tab.count()) {
  await tab.first().click();
  await p.waitForTimeout(900);
  await p.screenshot({ path: `${OUT}\\14-instance-l10n.png` });
  console.log("shot 14");
}
await b.close();
