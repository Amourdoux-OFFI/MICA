// UI 级功能检查：无头 Chromium 渲染真实前端产物 + 模拟 IPC，断言 + 截图。
import { chromium } from "playwright-core";
import fs from "node:fs";

const OUT = "D:\\Desktop\\DSH\\MICA\\.data\\ui";
fs.mkdirSync(OUT, { recursive: true });

const results = [];
function check(name, ok, detail = "") {
  results.push({ name, ok: !!ok, detail });
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? " :: " + detail : ""}`);
}

const browser = await chromium.launch({ channel: "chrome", headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 820 } });
const errors = [];
page.on("console", (m) => {
  if (m.type() === "error") errors.push(m.text());
});
page.on("pageerror", (e) => errors.push(e.message));

async function shot(name) {
  await page.waitForTimeout(450);
  await page.screenshot({ path: `${OUT}\\${name}.png` });
}

await page.goto("http://localhost:4173/", { waitUntil: "networkidle" });
await page.waitForTimeout(900);

// ---------- 首页 ----------
check("home 标题", (await page.textContent("h1"))?.includes("启动"));
const cards = await page.locator("[data-testid=instance-card]").count();
check("实例卡片数量=2", cards === 2, `cards=${cards}`);
check("实例名称可见", (await page.getByText("1.20.1 Fabric 0.16.10").count()) >= 1);
check("导入服务端包按钮", (await page.getByRole("button", { name: "导入服务端包" }).count()) === 1);
await shot("u01-home");

// ---------- 下载 / Mod ----------
await page.locator("aside button", { hasText: "下载" }).first().click();
await page.waitForTimeout(500);
const tabs = await page.locator("button.tab").count();
check("下载页 Tab 数量=7", tabs === 7, `tabs=${tabs}`);
await page.locator("button.tab", { hasText: "Mod" }).first().click();
await page.waitForTimeout(700);
const hitCards = await page.locator("button.card").count();
check("Mod 搜索结果 >=3", hitCards >= 3, `hits=${hitCards}`);
await shot("u02-mod-list");

// 空结果
const search = page.locator('input[placeholder*="搜索"]').first();
await search.fill("zzz");
await page.waitForTimeout(700);
check("空结果提示", (await page.getByText("没有找到结果").count()) >= 1);
await shot("u03-empty-search");
await search.fill("sodium");
await page.waitForTimeout(700);

// 项目详情
await page.locator("button.card").first().click();
await page.waitForTimeout(700);
check("项目详情标题", (await page.locator("h3", { hasText: "Sodium" }).count()) >= 1);
const verRows = await page.locator("div", { hasText: "0.5.13+mc1.20.1" }).count();
check("版本列表出现", verRows >= 1, `rows=${verRows}`);
await shot("u04-project-detail");
await page.keyboard.press("Escape");
await page.waitForTimeout(400);

// ---------- 游戏版本 ----------
await page.locator("button.tab", { hasText: "游戏版本" }).click();
await page.waitForTimeout(800);
const groups = await page.locator("div.grid > div.scroll-y button").count();
check("版本大版本分组 >=8", groups >= 8, `groups=${groups}`);
await shot("u05-versions");
await page.locator("button", { hasText: "安装" }).first().click();
await page.waitForTimeout(600);
check("安装向导出现", (await page.getByText("加载器", { exact: false }).count()) >= 1);
await page.locator("button.chip", { hasText: "Fabric" }).click();
await page.waitForTimeout(900);
const loaderOptions = await page.locator("select option").count();
check("Fabric 历史版本可选", loaderOptions >= 2, `options=${loaderOptions}`);
await shot("u06-install-wizard");
await page.keyboard.press("Escape");
await page.waitForTimeout(300);

// ---------- CurseForge（真实 Tab） ----------
await page.locator("button.tab", { hasText: "CurseForge" }).click();
await page.waitForTimeout(600);
check("CF 搜索框与来源", (await page.getByPlaceholder(/project infinity/).count()) >= 1 && (await page.getByText(/来源 CurseForge/).count()) >= 1);
await shot("u07-curseforge");

// ---------- 服务端 ----------
await page.locator("aside button", { hasText: "服务端" }).first().click();
await page.waitForTimeout(700);
const srvCards = await page.locator("[data-testid=server-card]").count();
check("服务端卡片 >=2", srvCards >= 2, `cards=${srvCards}`);
check("未同意 EULA 标记", (await page.getByText("未同意 EULA").count()) >= 1);
await shot("u08-servers");
await page.locator("button", { hasText: "控制台" }).first().click();
await page.waitForTimeout(700);
check("控制台含 Done 日志", (await page.getByText("Done (18.532s)").count()) >= 1);
await shot("u09-server-console");
await page.keyboard.press("Escape");
await page.waitForTimeout(300);

// ---------- 实例 / 汉化 ----------
await page.locator("aside button", { hasText: "实例" }).first().click();
await page.waitForTimeout(600);
const instCard = page.locator("button.card").first();
if (await instCard.count()) {
  await instCard.click();
  await page.waitForTimeout(600);
}
await page.locator("button.tab", { hasText: "汉化" }).click();
await page.waitForTimeout(600);
check("汉化记录可见", (await page.getByText("1.20.1 整合包汉化 v3").count()) >= 1);
await shot("u10-instance-l10n");
await page.locator("button", { hasText: "撤销" }).first().click();
await page.waitForTimeout(600);
check("撤销后出现提示", (await page.getByText("已撤销该汉化").count()) >= 1);
await shot("u11-l10n-rollback");

// ---------- 设置 / 诊断 / 主题 ----------
await page.locator("aside button", { hasText: "设置" }).first().click();
await page.waitForTimeout(600);
await page.getByRole("button", { name: "运行自检" }).click();
await page.waitForTimeout(900);
const checkRows = await page.getByText("WebView2 运行时").count();
check("自检结果渲染", checkRows >= 1);
await shot("u12-settings-doctor");
await page.getByRole("button", { name: "浅色" }).click();
await page.waitForTimeout(400);
const isLight = await page.evaluate(() => document.documentElement.classList.contains("light"));
check("主题切换生效", isLight);
await shot("u13-settings-light");
await page.getByRole("button", { name: "深色" }).click();
await page.waitForTimeout(300);

// ---------- 小窗口无横向溢出 ----------
await page.setViewportSize({ width: 1024, height: 700 });
await page.waitForTimeout(500);
const overflow = await page.evaluate(
  () => document.documentElement.scrollWidth - window.innerWidth
);
check("1024 宽无横向溢出", overflow <= 2, `overflow=${overflow}px`);
await shot("u14-narrow");


// ---------- 窄窗口逐页检查（1024） ----------
async function widthOk(label) {
  const over = await page.evaluate(
    () => document.documentElement.scrollWidth - window.innerWidth
  );
  check(`1024 ${label} 无溢出`, over <= 2, `overflow=${over}px`);
}
await page.setViewportSize({ width: 1024, height: 700 });
for (const [label, nav] of [
  ["首页", "启动"],
  ["下载", "下载"],
  ["服务端", "服务端"],
  ["设置", "设置"],
]) {
  await page.locator("aside button", { hasText: nav }).first().click();
  await page.waitForTimeout(450);
  await widthOk(label);
}

// 实例页 + 汉化页窄窗
await page.locator("aside button", { hasText: "实例" }).first().click();
await page.waitForTimeout(500);
const narrowCard = page.locator("[data-testid=instance-card], button.card").first();
if (await narrowCard.count()) {
  await narrowCard.click();
  await page.waitForTimeout(500);
}
await widthOk("实例详情");

// 弹窗不超出视口
await page.locator("aside button", { hasText: "下载" }).first().click();
await page.waitForTimeout(500);
await page.locator("button.tab", { hasText: "Mod" }).first().click();
await page.waitForTimeout(600);
await page.locator("button.card").first().click();
await page.waitForTimeout(600);
const dialogBox = await page.evaluate(() => {
  const el = document.querySelector('div[class*="max-h-[86vh]"]');
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { w: r.width, h: r.height, vw: window.innerWidth, vh: window.innerHeight };
});
check(
  "弹窗不超出视口",
  !!dialogBox && dialogBox.w <= dialogBox.vw && dialogBox.h <= dialogBox.vh,
  dialogBox ? `${Math.round(dialogBox.w)}x${Math.round(dialogBox.h)}` : "未找到弹窗"
);
await page.keyboard.press("Escape");

// 运行中的服务端：应显示"停止"
await page.locator("aside button", { hasText: "服务端" }).first().click();
await page.waitForTimeout(500);
check("运行中服务端显示停止", (await page.getByText("停止", { exact: true }).count()) >= 1);

// 减少动态效果
await page.emulateMedia({ reducedMotion: "reduce" });
const dur = await page.evaluate(() => {
  const el = document.querySelector("button");
  return el ? getComputedStyle(el).transitionDuration : "0s";
});
check("遵循减少动态效果", dur === "0.00001s" || dur === "0s" || parseFloat(dur) < 0.01, `transition=${dur}`);
await page.emulateMedia({ reducedMotion: "no-preference" });
// CurseForge Tab（mock IPC）
await page.setViewportSize({ width: 1280, height: 800 });
await page.locator("aside button", { hasText: "下载" }).first().click();
await page.waitForTimeout(500);
await page.locator("button.tab", { hasText: "CurseForge" }).first().click();
await page.waitForTimeout(1500);
const cfCards = await page.locator("button.card").count();
check("CF 搜索结果渲染", cfCards >= 1, `cards=${cfCards}`);
if (cfCards > 0) {
  await page.locator("button.card").first().click();
  await page.waitForTimeout(900);
  check("CF 详情含安装按钮", (await page.getByText("安装", { exact: true }).count()) >= 1);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
}
check("无前端报错", errors.length === 0, errors.slice(0, 2).join(" | "));

await browser.close();
const pass = results.filter((r) => r.ok).length;
fs.writeFileSync(`${OUT}\\summary.json`, "\uFEFF" + JSON.stringify(results, null, 2), "utf8");
console.log(`\nUI CHECK ${pass}/${results.length} passed -> ${OUT}`);
process.exit(pass === results.length ? 0 : 1);
