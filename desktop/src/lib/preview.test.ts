import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createDestinationPreview,
  type PreviewFetcher,
  type PreviewInput,
} from "@/lib/preview";
import type { DestinationPreview } from "@/lib/sidecar";

/** Controllable promise standing in for the Tauri invoke. */
function deferred() {
  let resolve!: (v: DestinationPreview) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<DestinationPreview>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function input(
  source: string,
  destination: string,
  keepRootFolder = false,
): PreviewInput {
  return { source, destination, keepRootFolder };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("createDestinationPreview", () => {
  it("空输入不发起请求并保持等待选择状态", () => {
    const fetcher = vi.fn<PreviewFetcher>();
    const preview = createDestinationPreview(fetcher);
    preview.setInput(input("", ""));
    expect(preview.getState()).toEqual({ phase: "idle" });
    expect(fetcher).not.toHaveBeenCalled();
  });

  it("单侧为空同样视为不完整输入", () => {
    const fetcher = vi.fn<PreviewFetcher>();
    const preview = createDestinationPreview(fetcher);
    preview.setInput(input("D:\\vault", "  "));
    expect(preview.getState()).toEqual({ phase: "idle" });
    expect(fetcher).not.toHaveBeenCalled();
  });

  it("完整输入先进入解析中，防抖到点后请求一次", async () => {
    const fetcher = vi.fn<PreviewFetcher>(() =>
      Promise.resolve({ target: "E:\\out", sourceKind: "directory" }),
    );
    const preview = createDestinationPreview(fetcher);
    preview.setInput(input("D:\\vault", "E:\\out"));
    expect(preview.getState()).toEqual({ phase: "pending" });
    expect(fetcher).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(150);
    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledWith(input("D:\\vault", "E:\\out"));
    expect(preview.getState()).toEqual({
      phase: "ready",
      target: "E:\\out",
      sourceKind: "directory",
    });
  });

  it("防抖窗口内的连续输入合并为最后一次", async () => {
    const fetcher = vi.fn<PreviewFetcher>(() =>
      Promise.resolve({ target: "T", sourceKind: "directory" }),
    );
    const preview = createDestinationPreview(fetcher);
    preview.setInput(input("A", "OUT"));
    await vi.advanceTimersByTimeAsync(100);
    preview.setInput(input("B", "OUT"));
    await vi.advanceTimersByTimeAsync(150);

    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledWith(input("B", "OUT"));
  });

  it("输入变化后，旧请求的迟到结果不得覆盖新输入", async () => {
    const first = deferred();
    const calls: Array<{ promise: Promise<DestinationPreview> }> = [];
    const fetcher: PreviewFetcher = (i) => {
      const d = i.source === "A" ? first : deferred();
      calls.push(d);
      return d.promise;
    };
    const preview = createDestinationPreview(fetcher);

    preview.setInput(input("A", "OUT"));
    await vi.advanceTimersByTimeAsync(150); // A 的请求已发出
    preview.setInput(input("B", "OUT")); // 输入变化：A 的结果即刻作废
    await vi.advanceTimersByTimeAsync(150); // B 的请求已发出

    first.resolve({ target: "A-OLD", sourceKind: "directory" }); // A 迟到返回
    await vi.advanceTimersByTimeAsync(0);
    expect(preview.getState()).toEqual({ phase: "pending" }); // 未被旧结果污染

    (calls[1] as unknown as {
      resolve: (v: DestinationPreview) => void;
    }).resolve({
      target: "B-NEW",
      sourceKind: "file",
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(preview.getState()).toEqual({
      phase: "ready",
      target: "B-NEW",
      sourceKind: "file",
    });
  });

  it("就绪后清空输入会回到等待选择，不残留旧路径", async () => {
    const fetcher = vi.fn<PreviewFetcher>(() =>
      Promise.resolve({ target: "E:\\out", sourceKind: "directory" }),
    );
    const preview = createDestinationPreview(fetcher);
    preview.setInput(input("D:\\vault", "E:\\out"));
    await vi.advanceTimersByTimeAsync(150);
    expect(preview.getState().phase).toBe("ready");

    preview.setInput(input("", "E:\\out"));
    expect(preview.getState()).toEqual({ phase: "idle" });
  });

  it("失败结果进入失败态；修改输入后重新解析并清掉失败", async () => {
    const fetcher = vi
      .fn<PreviewFetcher>()
      .mockRejectedValueOnce(new Error("boom"))
      .mockResolvedValue({ target: "E:\\out2", sourceKind: "other" });
    const preview = createDestinationPreview(fetcher);

    preview.setInput(input("D:\\vault", "E:\\out"));
    await vi.advanceTimersByTimeAsync(150);
    expect(preview.getState()).toMatchObject({ phase: "failed" });

    preview.setInput(input("D:\\vault2", "E:\\out"));
    expect(preview.getState()).toEqual({ phase: "pending" });
    await vi.advanceTimersByTimeAsync(150);
    expect(preview.getState()).toEqual({
      phase: "ready",
      target: "E:\\out2",
      sourceKind: "other",
    });
  });

  it("切换保留根文件夹即时失效旧预览并触发重新解析", async () => {
    const fetcher = vi.fn<PreviewFetcher>(() =>
      Promise.resolve({ target: "T", sourceKind: "directory" }),
    );
    const preview = createDestinationPreview(fetcher);
    preview.setInput(input("D:\\vault", "E:\\out", false));
    await vi.advanceTimersByTimeAsync(150);
    expect(preview.getState().phase).toBe("ready");

    preview.setInput(input("D:\\vault", "E:\\out", true));
    expect(preview.getState()).toEqual({ phase: "pending" });
    await vi.advanceTimersByTimeAsync(150);
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher).toHaveBeenLastCalledWith(input("D:\\vault", "E:\\out", true));
  });

  it("相同输入的重复 setInput 不重复请求也不闪回解析中", async () => {
    const fetcher = vi.fn<PreviewFetcher>(() =>
      Promise.resolve({ target: "T", sourceKind: "directory" }),
    );
    const preview = createDestinationPreview(fetcher);
    preview.setInput(input("S", "D"));
    await vi.advanceTimersByTimeAsync(150);
    expect(preview.getState().phase).toBe("ready");

    const before = preview.getState();
    preview.setInput(input("S", "D")); // 语言/尺寸等重渲染以相同值调用
    expect(preview.getState()).toBe(before);
    await vi.advanceTimersByTimeAsync(150);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it("dispose 取消挂起的防抖并作废在途结果", async () => {
    const d = deferred();
    const fetcher: PreviewFetcher = () => d.promise;
    const preview = createDestinationPreview(fetcher);
    preview.setInput(input("A", "OUT"));
    await vi.advanceTimersByTimeAsync(150); // 请求已发出
    preview.dispose();

    d.resolve({ target: "LATE", sourceKind: "directory" });
    await vi.advanceTimersByTimeAsync(0);
    expect(preview.getState()).toEqual({ phase: "pending" });
  });

  it("dispose 后相同输入的重放会重新解析（StrictMode 重挂载序列）", async () => {
    // StrictMode 挂载 → 卸载（dispose）→ 用相同 props 重挂载：重放的
    // setInput 不得被"输入未变"短路，否则预览永远停在初始态（烟测缺陷 A）。
    const fetcher = vi.fn<PreviewFetcher>(() =>
      Promise.resolve({ target: "T", sourceKind: "directory" }),
    );
    const preview = createDestinationPreview(fetcher);

    preview.setInput(input("S", "D")); // 首次挂载
    await vi.advanceTimersByTimeAsync(100); // 防抖窗口内
    preview.dispose(); // 卸载：timer 被清、代际作废

    preview.setInput(input("S", "D")); // 重挂载，值与之前完全相同
    expect(preview.getState()).toEqual({ phase: "pending" });
    await vi.advanceTimersByTimeAsync(150);
    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(preview.getState()).toMatchObject({ phase: "ready", target: "T" });
  });

  it("订阅者在状态变化时被通知，退订后不再收到", async () => {
    const fetcher = vi.fn<PreviewFetcher>(() =>
      Promise.resolve({ target: "T", sourceKind: "directory" }),
    );
    const preview = createDestinationPreview(fetcher);
    const seen: string[] = [];
    const unsub = preview.subscribe(() => seen.push(preview.getState().phase));

    preview.setInput(input("A", "OUT")); // idle → pending
    await vi.advanceTimersByTimeAsync(150); // → ready
    unsub();
    preview.setInput(input("B", "OUT")); // 不再通知

    expect(seen).toEqual(["pending", "ready"]);
  });
});
