import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { createSurfaceStore } from "@/core";
import { shadcnKit } from "@/kits/shadcn";
import { htmlKit } from "@/kits/html";
import { A2UIProvider, Surface } from "./index";

/**
 * M3：验证「协议核心 + 可插拔 ComponentKit」的缝。
 *
 * 同一 store / 同一协议树，仅切换 `<A2UIProvider kit={...}>` 的 kit prop：
 * - 渲染实现随之切换（shadcn ⇄ 纯 HTML）；
 * - Data Model / 输入值跨切换保留（状态活在 store，不在 kit）；
 * - 协议核心（store）与渲染核心（renderNode/Provider）零改动即支持。
 */
function makeStore() {
  const store = createSurfaceStore();
  store.ingest({
    version: "v1.0",
    createSurface: {
      surfaceId: "s1",
      catalogId: "basic",
      dataModel: { form: { name: "", agree: false } },
      components: [
        { id: "root", component: "Card", child: "col" },
        { id: "col", component: "Column", children: ["title", "name", "agree"] },
        { id: "title", component: "Text", text: "表单标题" },
        {
          id: "name",
          component: "TextField",
          value: { path: "/form/name" },
          label: "姓名",
        },
        {
          id: "agree",
          component: "CheckBox",
          value: { path: "/form/agree" },
          label: "同意",
        },
      ],
    },
  });
  return store;
}

describe("M3 kit 可切换", () => {
  it("同一协议树，两个 kit 渲染出不同实现（标记互斥），但同一内容", () => {
    const store = makeStore();
    const { container, rerender } = render(
      <A2UIProvider store={store} kit={shadcnKit}>
        <Surface surfaceId="s1" />
      </A2UIProvider>,
    );
    // shadcn：a2ui-* 标记在、html-* 标记不在
    expect(container.querySelector('[data-slot="a2ui-card"]')).toBeTruthy();
    expect(container.querySelector('[data-kit^="html"]')).toBeNull();
    expect(screen.getByText("表单标题")).toBeInTheDocument();

    // 仅换 kit prop
    rerender(
      <A2UIProvider store={store} kit={htmlKit}>
        <Surface surfaceId="s1" />
      </A2UIProvider>,
    );
    // html：html-* 标记在、a2ui-* 标记不在
    expect(container.querySelector('[data-kit="html-card"]')).toBeTruthy();
    expect(container.querySelector('[data-slot="a2ui-card"]')).toBeNull();
    // 同一协议树 → 同样渲染出标题
    expect(screen.getByText("表单标题")).toBeInTheDocument();
  });

  it("切换 kit 后 Data Model 与输入值保留", async () => {
    const user = userEvent.setup();
    const store = makeStore();
    const { rerender } = render(
      <A2UIProvider store={store} kit={shadcnKit}>
        <Surface surfaceId="s1" />
      </A2UIProvider>,
    );

    // 在 shadcn kit 下输入 + 勾选
    await user.type(screen.getByLabelText("姓名"), "张三");
    await user.click(screen.getByLabelText("同意"));
    expect(store.getDataValue("s1", "/form/name")).toBe("张三");
    expect(store.getDataValue("s1", "/form/agree")).toBe(true);

    // 切换到 html kit —— 同一 store 实例
    rerender(
      <A2UIProvider store={store} kit={htmlKit}>
        <Surface surfaceId="s1" />
      </A2UIProvider>,
    );

    // html kit 的输入渲染出保留的状态
    expect((screen.getByLabelText("姓名") as HTMLInputElement).value).toBe(
      "张三",
    );
    expect((screen.getByLabelText("同意") as HTMLInputElement).checked).toBe(
      true,
    );
    // store 状态未受 kit 切换影响
    expect(store.getDataValue("s1", "/form/name")).toBe("张三");
    expect(store.getDataValue("s1", "/form/agree")).toBe(true);
  });

  it("切换回 shadcn 仍保留状态（双向可切换）", async () => {
    const user = userEvent.setup();
    const store = makeStore();
    const { rerender } = render(
      <A2UIProvider store={store} kit={htmlKit}>
        <Surface surfaceId="s1" />
      </A2UIProvider>,
    );
    await user.type(screen.getByLabelText("姓名"), "李四");

    rerender(
      <A2UIProvider store={store} kit={shadcnKit}>
        <Surface surfaceId="s1" />
      </A2UIProvider>,
    );
    expect((screen.getByLabelText("姓名") as HTMLInputElement).value).toBe(
      "李四",
    );
  });
});
