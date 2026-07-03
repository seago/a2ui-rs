import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { Image } from "./Image";
import { Icon } from "./Icon";
import { Video } from "./Video";
import { AudioPlayer } from "./AudioPlayer";
import { Divider } from "./Divider";

describe("M2 显示组件", () => {
  it("Image 渲染 src 与 alt", () => {
    const { container } = render(
      <Image url="https://x/y.png" fit="contain" variant="avatar" />,
    );
    const img = container.querySelector("img")!;
    expect(img).toHaveAttribute("src", "https://x/y.png");
    expect(img).toHaveAttribute("alt", "avatar");
    expect(img.className).toContain("object-contain");
  });

  it("Icon 渲染已知图标（svg + aria-label）", () => {
    render(<Icon name="home" />);
    expect(screen.getByRole("img", { name: "home" })).toBeInTheDocument();
  });

  it("Icon 未知名回退但仍带 aria-label", () => {
    render(<Icon name="totally-unknown" />);
    expect(
      screen.getByRole("img", { name: "totally-unknown" }),
    ).toBeInTheDocument();
  });

  it("Video 有 controls 与 src", () => {
    const { container } = render(<Video url="v.mp4" posterUrl="p.png" />);
    const v = container.querySelector("video")!;
    expect(v).toHaveAttribute("src", "v.mp4");
    expect(v).toHaveAttribute("poster", "p.png");
    expect(v.hasAttribute("controls")).toBe(true);
  });

  it("AudioPlayer 有 controls、src 与描述", () => {
    const { container } = render(<AudioPlayer url="a.mp3" description="曲目" />);
    const a = container.querySelector("audio")!;
    expect(a).toHaveAttribute("src", "a.mp3");
    expect(a.hasAttribute("controls")).toBe(true);
    expect(screen.getByText("曲目")).toBeInTheDocument();
  });

  it("Divider 为 role=separator", () => {
    render(<Divider />);
    expect(screen.getByRole("separator")).toBeInTheDocument();
  });
});
