import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { createSurfaceStore } from "@/core";
import { shadcnKit } from "@/kits/shadcn";
import { A2UIProvider, Surface } from "./index";

/**
 * M2 端到端集成：真实 store（协议核心）+ V 渲染核心 + shadcn kit，
 * 用一条 createSurface 覆盖新组件（Divider/Image/CheckBox/List 模板/Tabs/Modal），
 * 验证渲染与交互回写贯通。
 */
function mountDemo() {
  const store = createSurfaceStore();
  store.ingest({
    version: "v1.0",
    createSurface: {
      surfaceId: "s1",
      catalogId: "basic",
      dataModel: {
        form: { agree: false },
        items: [{ label: "苹果" }, { label: "香蕉" }],
      },
      components: [
        {
          id: "root",
          component: "Column",
          children: ["div1", "img1", "chk1", "list1", "tabs1", "modal1"],
        },
        { id: "div1", component: "Divider" },
        { id: "img1", component: "Image", url: "https://x/a.png", variant: "avatar" },
        {
          id: "chk1",
          component: "CheckBox",
          value: { path: "/form/agree" },
          label: "同意条款",
        },
        {
          id: "list1",
          component: "List",
          direction: "vertical",
          children: { template: "itemTpl", path: "/items" },
        },
        { id: "itemTpl", component: "Text", text: { path: "label" } },
        {
          id: "tabs1",
          component: "Tabs",
          tabs: [
            { title: "标签甲", child: "tabA" },
            { title: "标签乙", child: "tabB" },
          ],
        },
        { id: "tabA", component: "Text", text: "内容甲" },
        { id: "tabB", component: "Text", text: "内容乙" },
        { id: "modal1", component: "Modal", content: "mContent", trigger: "mTrigger" },
        { id: "mContent", component: "Text", text: "弹窗正文" },
        {
          id: "mTrigger",
          component: "Button",
          child: "mTriggerLabel",
          variant: "primary",
        },
        { id: "mTriggerLabel", component: "Text", text: "打开弹窗" },
      ],
    },
  });

  render(
    <A2UIProvider store={store} kit={shadcnKit}>
      <Surface surfaceId="s1" />
    </A2UIProvider>,
  );
  return store;
}

describe("M2 端到端集成（真实 store + V + shadcn kit）", () => {
  it("显示组件与 List 模板渲染", () => {
    mountDemo();
    expect(screen.getByRole("separator")).toBeInTheDocument();
    const img = screen.getByRole("img");
    expect(img).toHaveAttribute("src", "https://x/a.png");
    // List 模板从 /items 展开两项文本
    expect(screen.getByText("苹果")).toBeInTheDocument();
    expect(screen.getByText("香蕉")).toBeInTheDocument();
  });

  it("CheckBox 交互写回 Data Model", async () => {
    const user = userEvent.setup();
    const store = mountDemo();
    expect(store.getDataValue("s1", "/form/agree")).toBe(false);
    await user.click(screen.getByLabelText("同意条款"));
    expect(store.getDataValue("s1", "/form/agree")).toBe(true);
  });

  it("Tabs 默认首页并可切换", async () => {
    const user = userEvent.setup();
    mountDemo();
    expect(screen.getByText("内容甲")).toBeInTheDocument();
    expect(screen.queryByText("内容乙")).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "标签乙" }));
    expect(screen.getByText("内容乙")).toBeInTheDocument();
  });

  it("Modal 点击 trigger 打开内容", async () => {
    const user = userEvent.setup();
    mountDemo();
    expect(screen.queryByText("弹窗正文")).not.toBeInTheDocument();
    await user.click(screen.getByText("打开弹窗"));
    expect(screen.getByText("弹窗正文")).toBeInTheDocument();
  });
});
