import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { CheckError } from "@/contracts";
import { shadcnKit } from "./index";

const { Text, Button, TextField, Card, Placeholder } = shadcnKit;

describe("shadcnKit shape", () => {
  it("exposes a component for every known A2UI type", () => {
    expect(typeof shadcnKit.Text).toBe("function");
    expect(typeof shadcnKit.Button).toBe("function");
    expect(typeof shadcnKit.TextField).toBe("function");
    expect(typeof shadcnKit.Card).toBe("function");
    expect(typeof shadcnKit.Placeholder).toBe("function");
  });
});

describe("Text", () => {
  it("renders body text", () => {
    render(<Text text="hello world" variant="body" />);
    expect(screen.getByText("hello world")).toBeInTheDocument();
  });

  it("renders caption text with muted styling", () => {
    render(<Text text="a caption" variant="caption" />);
    const el = screen.getByText("a caption");
    expect(el).toBeInTheDocument();
    expect(el.className).toContain("text-muted-foreground");
  });

  it("does not apply muted styling to body", () => {
    render(<Text text="body copy" variant="body" />);
    expect(screen.getByText("body copy").className).not.toContain(
      "text-muted-foreground"
    );
  });
});

describe("Button", () => {
  it("renders its label as children", () => {
    render(
      <Button label="Click me" variant="default" disabled={false} onAction={() => {}} />
    );
    expect(screen.getByRole("button", { name: "Click me" })).toBeInTheDocument();
  });

  it("fires onAction when clicked", async () => {
    const onAction = vi.fn();
    render(
      <Button label="Go" variant="primary" disabled={false} onAction={onAction} />
    );
    await userEvent.click(screen.getByRole("button", { name: "Go" }));
    expect(onAction).toHaveBeenCalledTimes(1);
  });

  it("maps primary variant to a solid (bg-primary) button", () => {
    render(
      <Button label="Primary" variant="primary" disabled={false} onAction={() => {}} />
    );
    expect(screen.getByRole("button", { name: "Primary" }).className).toContain(
      "bg-primary"
    );
  });

  it("maps borderless variant to a ghost button (no solid background)", () => {
    render(
      <Button label="Ghost" variant="borderless" disabled={false} onAction={() => {}} />
    );
    const cls = screen.getByRole("button", { name: "Ghost" }).className;
    expect(cls).not.toContain("bg-primary");
    expect(cls).not.toContain("bg-secondary");
  });

  it("passes disabled through and does not fire onAction", async () => {
    const onAction = vi.fn();
    render(
      <Button label="Nope" variant="default" disabled={true} onAction={onAction} />
    );
    const btn = screen.getByRole("button", { name: "Nope" });
    expect(btn).toBeDisabled();
    await userEvent.click(btn);
    expect(onAction).not.toHaveBeenCalled();
  });
});

describe("TextField", () => {
  it("renders a controlled value", () => {
    render(
      <TextField
        value="typed"
        onChange={() => {}}
        variant="shortText"
        disabled={false}
        errors={[]}
      />
    );
    expect(screen.getByDisplayValue("typed")).toBeInTheDocument();
  });

  it("fires onChange with the raw string value on input", async () => {
    const onChange = vi.fn();
    render(
      <TextField
        value=""
        onChange={onChange}
        variant="shortText"
        disabled={false}
        errors={[]}
      />
    );
    await userEvent.type(screen.getByRole("textbox"), "x");
    expect(onChange).toHaveBeenCalledWith("x");
  });

  it("renders an associated label when provided", () => {
    render(
      <TextField
        value=""
        onChange={() => {}}
        label="Your name"
        variant="shortText"
        disabled={false}
        errors={[]}
      />
    );
    expect(screen.getByLabelText("Your name")).toBeInTheDocument();
  });

  it("renders a textarea for longText variant", () => {
    render(
      <TextField
        value="para"
        onChange={() => {}}
        variant="longText"
        disabled={false}
        errors={[]}
      />
    );
    expect(screen.getByRole("textbox").tagName.toLowerCase()).toBe("textarea");
  });

  it("renders a password input for obscured variant", () => {
    const { container } = render(
      <TextField
        value="secret"
        onChange={() => {}}
        variant="obscured"
        disabled={false}
        errors={[]}
      />
    );
    const input = container.querySelector("input");
    expect(input).toHaveAttribute("type", "password");
  });

  it("renders a number input for number variant", () => {
    render(
      <TextField
        value="42"
        onChange={() => {}}
        variant="number"
        disabled={false}
        errors={[]}
      />
    );
    expect(screen.getByRole("spinbutton")).toBeInTheDocument();
  });

  it("disables the input when disabled", () => {
    render(
      <TextField
        value=""
        onChange={() => {}}
        variant="shortText"
        disabled={true}
        errors={[]}
      />
    );
    expect(screen.getByRole("textbox")).toBeDisabled();
  });

  it("shows an error state and message when errors present", () => {
    const errors: CheckError[] = [
      { message: "Required field", componentId: "c1", checkIndex: 0 },
    ];
    render(
      <TextField
        value=""
        onChange={() => {}}
        variant="shortText"
        disabled={false}
        errors={errors}
      />
    );
    expect(screen.getByText("Required field")).toBeInTheDocument();
    expect(screen.getByRole("textbox")).toHaveAttribute("aria-invalid", "true");
  });

  it("has no aria-invalid when there are no errors", () => {
    render(
      <TextField
        value=""
        onChange={() => {}}
        variant="shortText"
        disabled={false}
        errors={[]}
      />
    );
    expect(screen.getByRole("textbox")).not.toHaveAttribute(
      "aria-invalid",
      "true"
    );
  });
});

describe("Card", () => {
  it("renders its children", () => {
    render(
      <Card>
        <span>card content</span>
      </Card>
    );
    expect(screen.getByText("card content")).toBeInTheDocument();
  });
});

describe("Placeholder", () => {
  it("displays the reason", () => {
    render(<Placeholder reason="unknown component: Widget" />);
    expect(screen.getByText("unknown component: Widget")).toBeInTheDocument();
  });
});
