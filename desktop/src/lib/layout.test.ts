import { describe, expect, it } from "vitest";

import {
  TALL_BREAKPOINT,
  WIDE_BREAKPOINT,
  classifyLayout,
} from "@/lib/layout";

describe("classifyLayout 宽高独立断点", () => {
  it("默认窗口 760×520：窄且矮（胶囊、无右栏）", () => {
    expect(classifyLayout(760, 520)).toEqual({ wide: false, tall: false });
  });

  it("最小窗口 560×480：同上", () => {
    expect(classifyLayout(560, 480)).toEqual({ wide: false, tall: false });
  });

  it("宽度临界：999 隐藏右栏、1000 出现", () => {
    expect(classifyLayout(999, 800).wide).toBe(false);
    expect(classifyLayout(1000, 800).wide).toBe(true);
  });

  it("高度临界：639 胶囊、640 展开", () => {
    expect(classifyLayout(800, 639).tall).toBe(false);
    expect(classifyLayout(800, 640).tall).toBe(true);
  });

  it("宽而矮：右栏与胶囊并存（两条件独立）", () => {
    expect(classifyLayout(1000, 520)).toEqual({ wide: true, tall: false });
    expect(classifyLayout(1920, 600)).toEqual({ wide: true, tall: false });
  });

  it("窄而高：高度充足也不出现右栏", () => {
    expect(classifyLayout(999, 900)).toEqual({ wide: false, tall: true });
  });

  it("断点常量锁定规格取值", () => {
    expect(WIDE_BREAKPOINT).toBe(1000);
    expect(TALL_BREAKPOINT).toBe(640);
  });
});
