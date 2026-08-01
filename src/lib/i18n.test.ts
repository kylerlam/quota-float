import { describe, expect, it } from "vitest";
import { copy, nextLanguage, normalizeLanguage } from "./i18n";

describe("language preferences", () => {
  it("defaults unknown values to Simplified Chinese", () => {
    expect(normalizeLanguage(undefined)).toBe("zh-CN");
    expect(normalizeLanguage("fr")).toBe("zh-CN");
  });

  it("supports Simplified Chinese, Traditional Chinese, and English", () => {
    expect(normalizeLanguage("zh-TW")).toBe("zh-TW");
    expect(normalizeLanguage("en")).toBe("en");
    expect(nextLanguage("zh-CN")).toBe("zh-TW");
    expect(nextLanguage("zh-TW")).toBe("en");
    expect(nextLanguage("en")).toBe("zh-CN");
  });

  it("keeps each widget state in the selected language", () => {
    expect(copy["zh-CN"].loadingQuota).toBe("正在读取额度");
    expect(copy["zh-TW"].loadingQuota).toBe("正在讀取額度");
    expect(copy.en.loadingQuota).toBe("Reading quota");
    expect(copy["zh-TW"].notSignedIn).toBe("未登入");
  });
});
