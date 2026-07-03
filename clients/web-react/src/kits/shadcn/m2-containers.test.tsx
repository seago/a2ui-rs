import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { List } from "./List";
import { Tabs } from "./Tabs";
import { Modal } from "./Modal";

describe("M2 容器组件", () => {
  it("List 按 direction 决定 flex 方向并渲染 children", () => {
    const { container, rerender } = render(
      <List direction="horizontal">
        <span>x</span>
      </List>,
    );
    const el = container.querySelector('[data-slot="a2ui-list"]')!;
    expect(el.className).toContain("flex-row");
    expect(screen.getByText("x")).toBeInTheDocument();

    rerender(
      <List direction="vertical">
        <span>x</span>
      </List>,
    );
    expect(
      container.querySelector('[data-slot="a2ui-list"]')!.className,
    ).toContain("flex-col");
  });

  it("Tabs 默认显示第一页，点击切换", async () => {
    const user = userEvent.setup();
    render(
      <Tabs
        tabs={[
          { title: "第一", content: <div>内容一</div> },
          { title: "第二", content: <div>内容二</div> },
        ]}
      />,
    );
    expect(screen.getByText("内容一")).toBeInTheDocument();
    expect(screen.queryByText("内容二")).not.toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "第二" }));
    expect(screen.getByText("内容二")).toBeInTheDocument();
    expect(screen.queryByText("内容一")).not.toBeInTheDocument();
  });

  it("Modal 点击 trigger 打开、关闭后隐藏 content", async () => {
    const user = userEvent.setup();
    render(
      <Modal
        trigger={<button>打开</button>}
        content={<div>机密内容</div>}
      />,
    );
    expect(screen.queryByText("机密内容")).not.toBeInTheDocument();

    await user.click(screen.getByText("打开"));
    expect(screen.getByText("机密内容")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "关闭" }));
    expect(screen.queryByText("机密内容")).not.toBeInTheDocument();
  });
});
