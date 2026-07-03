import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CheckBox } from "./CheckBox";
import { Slider } from "./Slider";
import { ChoicePicker } from "./ChoicePicker";
import { DateTimeInput } from "./DateTimeInput";

const OPTS = [
  { value: "a", label: "苹果" },
  { value: "b", label: "香蕉" },
];

describe("M2 输入组件", () => {
  it("CheckBox 点击触发 onChange(true)、label 关联、disabled 阻止", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { rerender } = render(
      <CheckBox checked={false} onChange={onChange} label="同意" disabled={false} />,
    );
    expect(screen.getByLabelText("同意")).toBeInTheDocument();
    await user.click(screen.getByRole("checkbox"));
    expect(onChange).toHaveBeenCalledWith(true);

    onChange.mockClear();
    rerender(
      <CheckBox checked={false} onChange={onChange} label="同意" disabled />,
    );
    await user.click(screen.getByRole("checkbox"));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("Slider 改值触发 onChange(number)", () => {
    const onChange = vi.fn();
    render(
      <Slider
        value={10}
        onChange={onChange}
        min={0}
        max={100}
        label="音量"
        disabled={false}
      />,
    );
    fireEvent.change(screen.getByRole("slider"), { target: { value: "42" } });
    expect(onChange).toHaveBeenCalledWith(42);
  });

  it("ChoicePicker 多选累加与取消", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { rerender } = render(
      <ChoicePicker
        value={[]}
        onChange={onChange}
        options={OPTS}
        variant="multipleSelection"
        displayStyle="checkbox"
        disabled={false}
      />,
    );
    await user.click(screen.getByLabelText("苹果"));
    expect(onChange).toHaveBeenCalledWith(["a"]);

    onChange.mockClear();
    rerender(
      <ChoicePicker
        value={["a"]}
        onChange={onChange}
        options={OPTS}
        variant="multipleSelection"
        displayStyle="checkbox"
        disabled={false}
      />,
    );
    await user.click(screen.getByLabelText("香蕉"));
    expect(onChange).toHaveBeenCalledWith(["a", "b"]);
    await user.click(screen.getByLabelText("苹果"));
    expect(onChange).toHaveBeenCalledWith([]);
  });

  it("ChoicePicker 互斥单选替换", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <ChoicePicker
        value={["a"]}
        onChange={onChange}
        options={OPTS}
        variant="mutuallyExclusive"
        displayStyle="checkbox"
        disabled={false}
      />,
    );
    await user.click(screen.getByLabelText("香蕉"));
    expect(onChange).toHaveBeenCalledWith(["b"]);
  });

  it("DateTimeInput 按 enableDate/enableTime 选择 input 类型并回传字符串", () => {
    const onChange = vi.fn();
    const { container, rerender } = render(
      <DateTimeInput
        value=""
        onChange={onChange}
        label="日期"
        enableDate
        enableTime={false}
        disabled={false}
      />,
    );
    let input = container.querySelector("input")!;
    expect(input.type).toBe("date");
    fireEvent.change(input, { target: { value: "2026-07-02" } });
    expect(onChange).toHaveBeenCalledWith("2026-07-02");

    rerender(
      <DateTimeInput
        value=""
        onChange={onChange}
        enableDate
        enableTime
        disabled={false}
      />,
    );
    input = container.querySelector("input")!;
    expect(input.type).toBe("datetime-local");

    rerender(
      <DateTimeInput
        value=""
        onChange={onChange}
        enableDate={false}
        enableTime
        disabled={false}
      />,
    );
    input = container.querySelector("input")!;
    expect(input.type).toBe("time");
  });
});
