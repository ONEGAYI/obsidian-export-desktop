import { describe, expect, it } from "vitest";

import { baseName, displayName } from "@/lib/naming";

describe("baseName 路径末段提取", () => {
  it("两种分隔符均切分", () => {
    expect(baseName("D:\\Vaults\\我的库")).toBe("我的库");
    expect(baseName("/home/user/vault")).toBe("vault");
    expect(baseName("D:\\混合/路径\\file.md")).toBe("file.md");
  });

  it("无分隔符返回原样", () => {
    expect(baseName("vault")).toBe("vault");
    expect(baseName("file.md")).toBe("file.md");
  });

  it("尾部分隔符产出空末段（调用方自行修剪）", () => {
    expect(baseName("D:\\Vaults\\我的库\\")).toBe("");
    expect(baseName("/home/user/vault/")).toBe("");
  });
});

describe("displayName 首页路径名称派生", () => {
  it("常规目录名", () => {
    expect(displayName("D:\\Vaults\\我的库")).toBe("我的库");
    expect(displayName("/home/user/vault")).toBe("vault");
  });

  it("尾部分隔符不导致名称空白", () => {
    expect(displayName("D:\\Vaults\\我的库\\")).toBe("我的库");
    expect(displayName("D:\\Vaults\\我的库\\\\")).toBe("我的库");
    expect(displayName("/home/user/vault/")).toBe("vault");
  });

  it("中文与空格保留", () => {
    expect(displayName("D:\\临时\\笔记 库")).toBe("笔记 库");
  });

  it("空与纯空白输入返回 null（呈现占位而非虚构名称）", () => {
    expect(displayName("")).toBeNull();
    expect(displayName("   ")).toBeNull();
    expect(displayName("\\")).toBeNull();
  });

  it("盘符根等无名路径回退为修剪后的输入", () => {
    expect(displayName("C:\\")).toBe("C:");
    expect(displayName("C:")).toBe("C:");
  });

  it("相对路径取末段", () => {
    expect(displayName("vault")).toBe("vault");
    expect(displayName("some/rel/path.md")).toBe("path.md");
  });
});
