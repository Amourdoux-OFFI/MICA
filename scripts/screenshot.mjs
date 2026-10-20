import { chromium } from "playwright-core";
import fs from "node:fs";

const OUT = "D:\\Desktop\\DSH\\MICA\\.data\\shots";
fs.mkdirSync(OUT, { recursive: true });

const browser = await chromium.launch({ channel: "chrome", headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 820 }, deviceScaleFactor: 1 });

page.on("console", (m) => {
  if (m.type() === "error") console.log("[console]", m.text());
});
page.on("pageerror", (e) => console.log("[pageerror]", e.message));

await page.goto("http://localhost:4173/", { waitUntil: "networkidle" });
await page.waitForTimeout(1200);

async function shot(name) {
  await page.waitForTimeout(700);
  await page.screenshot({ path: `${OUT}\\${name}.png` });
  console.log("shot", name);
}

await shot("01-home");

// 下载页（Mod）
await page.getByRole("button", { name: "下载", exact: true }).click();
await page.waitForTimeout(800);
await shot("02-download-modpack");

const modTab = page.locator("button.tab", { hasText: "Mod" }).first();
await modTab.click();
await page.waitForTimeout(900);
await shot("03-download-mod");

// 项目详情
await page.locator("button.card").first().click();
await page.waitForTimeout(900);
await shot("04-project-detail");
await page.keyboard.press("Escape");

// 游戏版本
await page.locator("button.tab", { hasText: "游戏版本" }).click();
await page.waitForTimeout(900);
await shot("05-versions");
await page.locator("button", { hasText: "安装" }).first().click();
await page.waitForTimeout(700);
await shot("06-version-install");
await page.keyboard.press("Escape");

// CurseForge 空壳
await page.locator("button.tab", { hasText: "CurseForge" }).click();
await page.waitForTimeout(700);
await shot("07-curseforge-shell");

// 实例详情
await page.getByRole("button", { name: "实例", exact: true }).click();
await page.waitForTimeout(800);
const card = page.locator("button.card").first();
if (await card.count()) {
  await card.click();
  await page.waitForTimeout(900);
  await shot("08-instance-detail");
  await page.locator("button.tab", { hasText: "Mod" }).click();
  await page.waitForTimeout(700);
  await shot("09-instance-mods");
}

// 联机空壳
await page.getByRole("button", { name: "联机", exact: true }).click();
await page.waitForTimeout(700);
await shot("10-multiplayer-shell");

// 设置
await page.getByRole("button", { name: "设置", exact: true }).click();
await page.waitForTimeout(900);
await shot("11-settings");


// 服务端页
await page.getByRole("button", { name: "服务端", exact: true }).click();
await page.waitForTimeout(900);
await shot("12-servers");

const consoleBtn = page.locator("button", { hasText: "控制台" }).first();
if (await consoleBtn.count()) {
  await consoleBtn.click();
  await page.waitForTimeout(800);
  await shot("13-server-console");
  await page.keyboard.press("Escape");
}

// 实例汉化
await page.getByRole("button", { name: "实例", exact: true }).click();
await page.waitForTimeout(600);
const instCard = page.locator("button.card").first();
if (await instCard.count()) {
  await instCard.click();
  await page.waitForTimeout(700);
  await page.locator("button.tab", { hasText: "汉化" }).click();
  await page.waitForTimeout(800);
  await shot("14-instance-l10n");
}
await browser.close();
console.log("done");
