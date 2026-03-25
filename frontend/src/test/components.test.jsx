import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Badge } from "../components/ui/Badge";
import { PanelBlock } from "../components/ui/PanelBlock";
import { SegmentedControl } from "../components/ui/SegmentedControl";
import { SectionTitle } from "../components/ui/SectionTitle";
import { MiniStat } from "../components/ui/MiniStat";
import { DetailItem } from "../components/ui/DetailItem";

describe("Badge", () => {
  it("renders children text", () => {
    render(<Badge>3 台已选</Badge>);
    expect(screen.getByText("3 台已选")).toBeInTheDocument();
  });

  it("applies tone class", () => {
    const { container } = render(<Badge tone="green">OK</Badge>);
    expect(container.firstChild).toHaveClass("badge", "tone-green");
  });

  it("defaults to blue tone", () => {
    const { container } = render(<Badge>默认</Badge>);
    expect(container.firstChild).toHaveClass("tone-blue");
  });
});

describe("PanelBlock", () => {
  it("renders title and children", () => {
    render(<PanelBlock title="设备控制"><p>内容</p></PanelBlock>);
    expect(screen.getByText("设备控制")).toBeInTheDocument();
    expect(screen.getByText("内容")).toBeInTheDocument();
  });

  it("has panel-block class", () => {
    const { container } = render(<PanelBlock title="T">X</PanelBlock>);
    expect(container.firstChild).toHaveClass("panel-block");
  });
});

describe("SegmentedControl", () => {
  const options = [
    { value: "a", label: "选项A" },
    { value: "b", label: "选项B" },
    { value: "c", label: "选项C" },
  ];

  it("renders all options", () => {
    render(<SegmentedControl options={options} value="a" onChange={() => {}} />);
    expect(screen.getByText("选项A")).toBeInTheDocument();
    expect(screen.getByText("选项B")).toBeInTheDocument();
    expect(screen.getByText("选项C")).toBeInTheDocument();
  });

  it("marks active option", () => {
    render(<SegmentedControl options={options} value="b" onChange={() => {}} />);
    expect(screen.getByText("选项B")).toHaveClass("active");
    expect(screen.getByText("选项A")).not.toHaveClass("active");
  });

  it("calls onChange on click", () => {
    const onChange = vi.fn();
    render(<SegmentedControl options={options} value="a" onChange={onChange} />);
    fireEvent.click(screen.getByText("选项C"));
    expect(onChange).toHaveBeenCalledWith("c");
  });
});

describe("SectionTitle", () => {
  it("renders title text", () => {
    render(<SectionTitle title="工具箱" />);
    expect(screen.getByText("工具箱")).toBeInTheDocument();
  });
});

describe("MiniStat", () => {
  it("renders label and value", () => {
    render(<MiniStat label="在线" value="12" />);
    expect(screen.getByText("在线")).toBeInTheDocument();
    expect(screen.getByText("12")).toBeInTheDocument();
  });
});

describe("DetailItem", () => {
  it("renders label and value", () => {
    render(<DetailItem label="序列号" value="emulator-5554" />);
    expect(screen.getByText("序列号")).toBeInTheDocument();
    expect(screen.getByText("emulator-5554")).toBeInTheDocument();
  });
});
