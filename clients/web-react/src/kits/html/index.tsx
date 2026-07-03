/**
 * 纯 HTML ComponentKit —— B2 的第二个组件库实现（M3）。
 *
 * 用最朴素的 HTML 元素实现 `@/contracts` 的 `ComponentKit` 契约，不依赖
 * shadcn / Tailwind。它的存在是为了证明「协议核心 + 可插拔 ComponentKit」
 * 的缝真的成立：`<A2UIProvider kit={htmlKit}>` 即可整库切换，协议核心（store）
 * 与渲染核心（renderNode/Provider）零改动，状态（Data Model）跨切换保留。
 *
 * 每个组件带 `data-kit="html-*"` 标记，便于与 shadcn kit（`data-slot="a2ui-*"`）区分。
 */
import { useId, useState } from "react";

import type { ComponentKit } from "@/contracts";

const Text: ComponentKit["Text"] = ({ text, variant }) => (
  <span data-kit="html-text" data-variant={variant}>
    {text}
  </span>
);

const Image: ComponentKit["Image"] = ({ url, fit, variant }) => (
  <img
    data-kit="html-image"
    src={url}
    alt={variant ?? "image"}
    style={{ objectFit: fit }}
  />
);

const Icon: ComponentKit["Icon"] = ({ name }) => (
  <span data-kit="html-icon" aria-label={name} role="img">
    [{name}]
  </span>
);

const Video: ComponentKit["Video"] = ({ url, posterUrl }) => (
  <video data-kit="html-video" src={url} poster={posterUrl} controls />
);

const AudioPlayer: ComponentKit["AudioPlayer"] = ({ url, description }) => (
  <span data-kit="html-audio">
    <audio src={url} controls />
    {description ? <small>{description}</small> : null}
  </span>
);

const Row: ComponentKit["Row"] = ({ children }) => (
  <div data-kit="html-row" style={{ display: "flex", flexDirection: "row", gap: 8 }}>
    {children}
  </div>
);

const Column: ComponentKit["Column"] = ({ children }) => (
  <div data-kit="html-column" style={{ display: "flex", flexDirection: "column", gap: 8 }}>
    {children}
  </div>
);

const List: ComponentKit["List"] = ({ children, direction }) => (
  <div
    data-kit="html-list"
    role="list"
    style={{
      display: "flex",
      flexDirection: direction === "horizontal" ? "row" : "column",
      gap: 8,
    }}
  >
    {children}
  </div>
);

const Card: ComponentKit["Card"] = ({ children }) => (
  <div data-kit="html-card" style={{ border: "1px solid #ccc", padding: 12 }}>
    {children}
  </div>
);

const Tabs: ComponentKit["Tabs"] = ({ tabs }) => {
  const [active, setActive] = useState(0);
  return (
    <div data-kit="html-tabs">
      <div role="tablist">
        {tabs.map((t, i) => (
          <button
            key={i}
            type="button"
            role="tab"
            aria-selected={i === active}
            onClick={() => setActive(i)}
          >
            {t.title}
          </button>
        ))}
      </div>
      <div role="tabpanel">{tabs[active]?.content ?? null}</div>
    </div>
  );
};

const Modal: ComponentKit["Modal"] = ({ content, trigger }) => {
  const [open, setOpen] = useState(false);
  return (
    <span data-kit="html-modal">
      <span onClick={() => setOpen(true)}>{trigger}</span>
      {open ? (
        <div role="dialog" aria-modal="true">
          <button type="button" aria-label="关闭" onClick={() => setOpen(false)}>
            ×
          </button>
          {content}
        </div>
      ) : null}
    </span>
  );
};

const Divider: ComponentKit["Divider"] = () => <hr data-kit="html-divider" />;

const Button: ComponentKit["Button"] = ({ label, variant, disabled, onAction }) => (
  <button
    data-kit="html-button"
    data-variant={variant}
    disabled={disabled}
    onClick={onAction}
  >
    {label}
  </button>
);

const TextField: ComponentKit["TextField"] = ({
  value,
  onChange,
  label,
  placeholder,
  variant,
  disabled,
  errors,
}) => {
  const id = useId();
  return (
    <span data-kit="html-textfield">
      {label ? <label htmlFor={id}>{label}</label> : null}
      {variant === "longText" ? (
        <textarea
          id={id}
          value={value}
          placeholder={placeholder}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
        />
      ) : (
        <input
          id={id}
          type={
            variant === "obscured"
              ? "password"
              : variant === "number"
                ? "number"
                : "text"
          }
          value={value}
          placeholder={placeholder}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
        />
      )}
      {errors.length > 0 ? <small role="alert">{errors[0].message}</small> : null}
    </span>
  );
};

const CheckBox: ComponentKit["CheckBox"] = ({ checked, onChange, label, disabled }) => {
  const id = useId();
  return (
    <span data-kit="html-checkbox">
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      {label ? <label htmlFor={id}>{label}</label> : null}
    </span>
  );
};

const ChoicePicker: ComponentKit["ChoicePicker"] = ({
  value,
  onChange,
  options,
  variant,
  disabled,
}) => {
  const name = useId();
  const multiple = variant === "multipleSelection";
  const toggle = (v: string) =>
    multiple
      ? onChange(value.includes(v) ? value.filter((x) => x !== v) : [...value, v])
      : onChange([v]);
  return (
    <div data-kit="html-choicepicker" role="group">
      {options.map((o) => (
        <label key={o.value}>
          <input
            type={multiple ? "checkbox" : "radio"}
            name={multiple ? undefined : name}
            checked={value.includes(o.value)}
            disabled={disabled}
            onChange={() => toggle(o.value)}
          />
          {o.label}
        </label>
      ))}
    </div>
  );
};

const Slider: ComponentKit["Slider"] = ({
  value,
  onChange,
  min,
  max,
  step,
  label,
  disabled,
}) => {
  const id = useId();
  return (
    <span data-kit="html-slider">
      {label ? <label htmlFor={id}>{label}</label> : null}
      <input
        id={id}
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
      />
    </span>
  );
};

const DateTimeInput: ComponentKit["DateTimeInput"] = ({
  value,
  onChange,
  label,
  enableDate,
  enableTime,
  min,
  max,
  disabled,
}) => {
  const id = useId();
  const type =
    enableDate && enableTime ? "datetime-local" : enableTime ? "time" : "date";
  return (
    <span data-kit="html-datetime">
      {label ? <label htmlFor={id}>{label}</label> : null}
      <input
        id={id}
        type={type}
        value={value}
        min={min}
        max={max}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
      />
    </span>
  );
};

const Placeholder: ComponentKit["Placeholder"] = ({ reason }) => (
  <div data-kit="html-placeholder">[{reason}]</div>
);

/** 纯 HTML 版 A2UI ComponentKit（B2 的第二个 kit，用于验证可切换）。 */
export const htmlKit: ComponentKit = {
  Text,
  Image,
  Icon,
  Video,
  AudioPlayer,
  Row,
  Column,
  List,
  Card,
  Tabs,
  Modal,
  Divider,
  Button,
  TextField,
  CheckBox,
  ChoicePicker,
  Slider,
  DateTimeInput,
  Placeholder,
};
