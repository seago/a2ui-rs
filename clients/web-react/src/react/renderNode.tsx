// tree-walker：NodeRef → ResolvedNode → ComponentKit 组件，并接线交互回传。
//
// 递归通过 <RenderNode> 组件（而非在循环里调用 hook）实现，规避 hooks 规则。

import type { ReactNode } from "react";

import type {
  Action,
  ActionEvent,
  CheckError,
  NodeRef,
  ResolvedNode,
  SurfaceId,
} from "@/contracts";

import { useA2UIContext } from "./context";
import type { A2UIContextValue } from "./context";

/**
 * 解析并渲染单个 (组件 × 作用域) 节点。
 *
 * @example
 * ```tsx
 * useRenderNode("s1", { componentId: "root", scope: { frames: [] } });
 * ```
 */
export function useRenderNode(surfaceId: SurfaceId, nodeRef: NodeRef): ReactNode {
  const ctx = useA2UIContext();
  const { store, kit } = ctx;

  const resolved = store.resolveNode(surfaceId, nodeRef);
  if (!resolved) {
    return <kit.Placeholder reason={`未解析的组件引用: ${nodeRef.componentId}`} />;
  }

  // 引用缺失 / 类型未知等：核心层已给出占位原因，优先渲染 Placeholder。
  if (resolved.placeholder) {
    return <kit.Placeholder reason={resolved.placeholder} />;
  }

  return renderResolved(ctx, surfaceId, nodeRef, resolved);
}

/**
 * 递归渲染节点的组件包装：children 映射为一组 `<RenderNode>`，形成组件级递归。
 */
export function RenderNode({
  surfaceId,
  nodeRef,
}: {
  surfaceId: SurfaceId;
  nodeRef: NodeRef;
}): ReactNode {
  return useRenderNode(surfaceId, nodeRef);
}

// ─── 内部实现 ────────────────────────────────────────────────────────────────

function renderChildren(surfaceId: SurfaceId, children: NodeRef[]): ReactNode {
  return children.map((childRef, i) => (
    <RenderNode
      key={`${childRef.componentId}:${i}`}
      surfaceId={surfaceId}
      nodeRef={childRef}
    />
  ));
}

function renderResolved(
  ctx: A2UIContextValue,
  surfaceId: SurfaceId,
  nodeRef: NodeRef,
  resolved: ResolvedNode,
): ReactNode {
  const { kit } = ctx;

  switch (resolved.component) {
    case "Text": {
      const Text = kit.Text;
      return (
        <Text
          text={asString(resolved.props.text)}
          variant={pick(resolved.props.variant, ["body", "caption"], "body")}
        />
      );
    }

    case "Button": {
      const Button = kit.Button;
      // label 内嵌子组件时递归渲染 children；否则用字面量 label。
      const label: ReactNode =
        resolved.children.length > 0
          ? renderChildren(surfaceId, resolved.children)
          : asString(resolved.props.label);
      return (
        <Button
          label={label}
          variant={pick(
            resolved.props.variant,
            ["default", "primary", "borderless"],
            "default",
          )}
          disabled={resolved.disabled ?? false}
          onAction={() => handleAction(ctx, surfaceId, nodeRef, resolved)}
        />
      );
    }

    case "TextField": {
      const TextField = kit.TextField;
      const path = resolved.bindingPath;
      const value = path ? ctx.store.getDataValue(surfaceId, path) : undefined;
      return (
        <TextField
          value={asString(value)}
          onChange={(v) => {
            if (path) ctx.store.setDataValue(surfaceId, path, v);
          }}
          label={optionalString(resolved.props.label)}
          placeholder={optionalString(resolved.props.placeholder)}
          variant={pick(
            resolved.props.variant,
            ["shortText", "number", "longText", "obscured"],
            "shortText",
          )}
          disabled={resolved.disabled ?? false}
          errors={(resolved.errors ?? []) as CheckError[]}
        />
      );
    }

    case "Card": {
      const Card = kit.Card;
      return <Card>{renderChildren(surfaceId, resolved.children)}</Card>;
    }

    case "Column": {
      const Column = kit.Column;
      return <Column>{renderChildren(surfaceId, resolved.children)}</Column>;
    }

    case "Row": {
      const Row = kit.Row;
      return <Row>{renderChildren(surfaceId, resolved.children)}</Row>;
    }

    case "List": {
      const List = kit.List;
      return (
        <List
          direction={pick(
            resolved.props.direction,
            ["vertical", "horizontal"],
            "vertical",
          )}
        >
          {renderChildren(surfaceId, resolved.children)}
        </List>
      );
    }

    case "Image": {
      const Image = kit.Image;
      return (
        <Image
          url={asString(resolved.props.url)}
          fit={optionalEnum(resolved.props.fit, ["contain", "cover", "fill"])}
          variant={optionalString(resolved.props.variant)}
        />
      );
    }

    case "Icon": {
      const Icon = kit.Icon;
      return <Icon name={asString(resolved.props.name)} />;
    }

    case "Video": {
      const Video = kit.Video;
      return (
        <Video
          url={asString(resolved.props.url)}
          posterUrl={optionalString(resolved.props.posterUrl)}
        />
      );
    }

    case "AudioPlayer": {
      const AudioPlayer = kit.AudioPlayer;
      return (
        <AudioPlayer
          url={asString(resolved.props.url)}
          description={optionalString(resolved.props.description)}
        />
      );
    }

    case "Divider": {
      const Divider = kit.Divider;
      return <Divider />;
    }

    case "Tabs": {
      const Tabs = kit.Tabs;
      const rawTabs = Array.isArray(resolved.props.tabs)
        ? resolved.props.tabs
        : [];
      const tabs = rawTabs.map((t, i) => {
        const childRef = resolved.children[i];
        return {
          title: asString(isObject(t) ? t.title : undefined),
          content: childRef ? (
            <RenderNode
              key={`${childRef.componentId}:${i}`}
              surfaceId={surfaceId}
              nodeRef={childRef}
            />
          ) : null,
        };
      });
      return <Tabs tabs={tabs} />;
    }

    case "Modal": {
      const Modal = kit.Modal;
      const [contentRef, triggerRef] = resolved.children;
      return (
        <Modal
          content={
            contentRef ? (
              <RenderNode surfaceId={surfaceId} nodeRef={contentRef} />
            ) : null
          }
          trigger={
            triggerRef ? (
              <RenderNode surfaceId={surfaceId} nodeRef={triggerRef} />
            ) : null
          }
        />
      );
    }

    case "CheckBox": {
      const CheckBox = kit.CheckBox;
      const path = resolved.bindingPath;
      const checked = path
        ? Boolean(ctx.store.getDataValue(surfaceId, path))
        : Boolean(resolved.props.checked);
      return (
        <CheckBox
          checked={checked}
          onChange={(b) => {
            if (path) ctx.store.setDataValue(surfaceId, path, b);
          }}
          label={optionalString(resolved.props.label)}
          disabled={resolved.disabled ?? false}
        />
      );
    }

    case "Slider": {
      const Slider = kit.Slider;
      const path = resolved.bindingPath;
      const value = path
        ? asNumber(ctx.store.getDataValue(surfaceId, path), 0)
        : asNumber(resolved.props.value, 0);
      return (
        <Slider
          value={value}
          onChange={(n) => {
            if (path) ctx.store.setDataValue(surfaceId, path, n);
          }}
          min={asNumber(resolved.props.min, 0)}
          max={asNumber(resolved.props.max, 100)}
          step={optionalNumber(resolved.props.steps ?? resolved.props.step)}
          label={optionalString(resolved.props.label)}
          disabled={resolved.disabled ?? false}
        />
      );
    }

    case "ChoicePicker": {
      const ChoicePicker = kit.ChoicePicker;
      const path = resolved.bindingPath;
      const value = path
        ? asStringList(ctx.store.getDataValue(surfaceId, path))
        : asStringList(resolved.props.value);
      return (
        <ChoicePicker
          value={value}
          onChange={(v) => {
            if (path) ctx.store.setDataValue(surfaceId, path, v);
          }}
          options={normalizeOptions(resolved.props.options)}
          variant={pick(
            resolved.props.variant,
            ["multipleSelection", "mutuallyExclusive"],
            "multipleSelection",
          )}
          displayStyle={pick(
            resolved.props.displayStyle,
            ["checkbox", "chips"],
            "checkbox",
          )}
          disabled={resolved.disabled ?? false}
        />
      );
    }

    case "DateTimeInput": {
      const DateTimeInput = kit.DateTimeInput;
      const path = resolved.bindingPath;
      const value = path
        ? asString(ctx.store.getDataValue(surfaceId, path))
        : asString(resolved.props.value);
      return (
        <DateTimeInput
          value={value}
          onChange={(s) => {
            if (path) ctx.store.setDataValue(surfaceId, path, s);
          }}
          label={optionalString(resolved.props.label)}
          enableDate={resolved.props.enableDate !== false}
          enableTime={Boolean(resolved.props.enableTime)}
          min={optionalString(resolved.props.min)}
          max={optionalString(resolved.props.max)}
          disabled={resolved.disabled ?? false}
        />
      );
    }

    default:
      return <kit.Placeholder reason={`未知组件类型: ${resolved.component}`} />;
  }
}

/**
 * 处理 Button 等交互组件的 action：
 * - Event：由 store.buildActionEnvelope 生成信封，交给 onClientMessage 回传。
 * - FunctionCall：本地注册函数调用（M1 结构到位，实际派发留 TODO）。
 */
function handleAction(
  ctx: A2UIContextValue,
  surfaceId: SurfaceId,
  nodeRef: NodeRef,
  resolved: ResolvedNode,
): void {
  const action = resolved.action;
  if (!action) return;

  if (isEventAction(action)) {
    const envelope = ctx.store.buildActionEnvelope(
      surfaceId,
      action,
      resolved.id,
      nodeRef.scope,
    );
    ctx.onClientMessage?.(envelope);
    return;
  }

  // TODO(M1): ActionFunctionCall —— 派发到本地注册函数，必要时回传 functionResponse。
}

/** Action 判别：含 `name` 为 Event，含 `call` 为本地函数调用。 */
function isEventAction(action: Action): action is ActionEvent {
  return "name" in action;
}

// ─── props 归一化辅助 ────────────────────────────────────────────────────────

function asString(value: unknown): string {
  if (value === null || value === undefined) return "";
  return typeof value === "string" ? value : String(value);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asNumber(value: unknown, fallback: number): number {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const n = Number(value);
    if (Number.isFinite(n)) return n;
  }
  return fallback;
}

function optionalNumber(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim() !== "") {
    const n = Number(value);
    if (Number.isFinite(n)) return n;
  }
  return undefined;
}

function optionalEnum<T extends string>(
  value: unknown,
  allowed: readonly T[],
): T | undefined {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : undefined;
}

/** 归一化为字符串数组：数组取字符串项；单个字符串包一层；否则空数组。 */
function asStringList(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.filter((v): v is string => typeof v === "string");
  }
  return typeof value === "string" ? [value] : [];
}

/** 归一化 ChoicePicker 选项：接受 string[] 或 {value,label}[]，统一为 {value,label}[]。 */
function normalizeOptions(value: unknown): { value: string; label: string }[] {
  if (!Array.isArray(value)) return [];
  return value.map((o) => {
    if (typeof o === "string") return { value: o, label: o };
    if (isObject(o)) {
      const v = asString(o.value);
      return { value: v, label: o.label !== undefined ? asString(o.label) : v };
    }
    return { value: "", label: "" };
  });
}

function optionalString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function pick<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}
