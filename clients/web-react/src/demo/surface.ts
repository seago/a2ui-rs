import type { ServerEnvelope } from "@/contracts";

/**
 * 示例用的一条 A2UI `createSurface` 消息，覆盖多数 Basic Catalog 组件
 * （展示 / 输入 / 容器）。DemoApp 会把它喂进协议核心 store，再用不同的
 * ComponentKit 渲染，从而展示「换 kit 换库、状态不变」。
 */
export const demoEnvelope: ServerEnvelope = {
  version: "v1.0",
  createSurface: {
    surfaceId: "demo",
    catalogId: "basic",
    sendDataModel: true,
    dataModel: {
      form: {
        name: "张三",
        email: "zhangsan@example.com",
        subscribe: true,
        volume: 60,
        fruits: ["apple", "orange"],
      },
    },
    components: [
      { id: "demo", component: "Card", child: "main_col" },
      {
        id: "main_col",
        component: "Column",
        children: [
          "title",
          "sub",
          "div1",
          "name",
          "email",
          "chk",
          "vol",
          "fruit_label",
          "fruits",
          "div2",
          "tabs",
          "actions",
          "modal",
        ],
      },
      { id: "title", component: "Text", text: "A2UI 组件示例" },
      {
        id: "sub",
        component: "Text",
        text: "同一套 A2UI 协议消息",
        variant: "caption",
      },
      { id: "div1", component: "Divider" },
      {
        id: "name",
        component: "TextField",
        value: { path: "/form/name" },
        label: "姓名",
        placeholder: "请输入姓名",
        variant: "shortText",
      },
      {
        id: "email",
        component: "TextField",
        value: { path: "/form/email" },
        label: "邮箱",
        placeholder: "you@example.com",
        variant: "shortText",
      },
      {
        id: "chk",
        component: "CheckBox",
        value: { path: "/form/subscribe" },
        label: "订阅产品通讯",
      },
      {
        id: "vol",
        component: "Slider",
        value: { path: "/form/volume" },
        label: "音量",
        min: 0,
        max: 100,
      },
      {
        id: "fruit_label",
        component: "Text",
        text: "喜欢的水果",
        variant: "caption",
      },
      {
        id: "fruits",
        component: "ChoicePicker",
        value: { path: "/form/fruits" },
        options: [
          { value: "apple", label: "苹果" },
          { value: "banana", label: "香蕉" },
          { value: "orange", label: "橙子" },
        ],
        variant: "multipleSelection",
        displayStyle: "chips",
      },
      { id: "div2", component: "Divider" },
      {
        id: "tabs",
        component: "Tabs",
        tabs: [
          { title: "概览", child: "tab1" },
          { title: "详情", child: "tab2" },
        ],
      },
      { id: "tab1", component: "Text", text: "这是概览标签页的内容。" },
      { id: "tab2", component: "Text", text: "这是详情标签页的内容。" },
      { id: "actions", component: "Row", children: ["bp", "bd", "bg"] },
      {
        id: "bp",
        component: "Button",
        child: "bp_l",
        variant: "primary",
        action: { event: { name: "submit" } },
      },
      { id: "bp_l", component: "Text", text: "提交" },
      {
        id: "bd",
        component: "Button",
        child: "bd_l",
        variant: "default",
        action: { event: { name: "cancel" } },
      },
      { id: "bd_l", component: "Text", text: "取消" },
      {
        id: "bg",
        component: "Button",
        child: "bg_l",
        variant: "borderless",
        action: { event: { name: "more" } },
      },
      { id: "bg_l", component: "Text", text: "更多" },
      {
        id: "modal",
        component: "Modal",
        trigger: "m_trig",
        content: "m_content",
      },
      {
        id: "m_trig",
        component: "Button",
        child: "mt_l",
        variant: "default",
        action: { event: { name: "noop" } },
      },
      { id: "mt_l", component: "Text", text: "打开对话框" },
      {
        id: "m_content",
        component: "Text",
        text: "这是一个由 A2UI Modal 渲染的对话框。",
      },
    ],
  },
};
