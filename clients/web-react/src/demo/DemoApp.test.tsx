import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DemoApp } from "./DemoApp";

/** 示例页冒烟测试：默认 shadcn、可切到 html、切换后表单状态保留。 */
describe("DemoApp", () => {
  it("默认 shadcn kit 渲染示例 surface", () => {
    const { container } = render(<DemoApp />);
    expect(container.querySelector('[data-slot="a2ui-card"]')).toBeTruthy();
    expect(container.querySelector('[data-kit^="html"]')).toBeNull();
    expect(screen.getByText("A2UI 组件示例")).toBeInTheDocument();
  });

  it("切换到 html kit 后换库但内容不变", async () => {
    const user = userEvent.setup();
    const { container } = render(<DemoApp />);
    await user.click(screen.getByRole("tab", { name: "纯 HTML kit" }));
    expect(container.querySelector('[data-kit="html-card"]')).toBeTruthy();
    expect(container.querySelector('[data-slot="a2ui-card"]')).toBeNull();
    expect(screen.getByText("A2UI 组件示例")).toBeInTheDocument();
  });

  it("切换 kit 后输入值保留（状态在 store）", async () => {
    const user = userEvent.setup();
    render(<DemoApp />);
    // 预置 dataModel 里 /form/name = 张三，两个 kit 都应展示
    expect((screen.getByLabelText("姓名") as HTMLInputElement).value).toBe(
      "张三",
    );
    await user.click(screen.getByRole("tab", { name: "纯 HTML kit" }));
    expect((screen.getByLabelText("姓名") as HTMLInputElement).value).toBe(
      "张三",
    );
  });
});
