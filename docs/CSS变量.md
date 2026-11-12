# CSS 变量（核心）

所有变量使用 `--dme-` 前缀，通过 `var(--name, fallback)` 提供默认值，不在组件根节点统一声明，保留父容器的变量继承。宿主可通过 `MindMap { class, style }` 或祖先元素覆盖。不同实例的祖先变量互不影响。

| 变量 | 默认值 | 范围 |
| --- | --- | --- |
| `--dme-background` | `#f8faf7` | 画布背景 |
| `--dme-text-color` | `#263b36` | 基础文本 |
| `--dme-font-family` | `system-ui, sans-serif` | 字体 |
| `--dme-font-size` | `15px` | 字号 |
| `--dme-line-height` | `1.55` | 节点行高 |
| `--dme-min-height` | `260px` | 容器最小高度 |
| `--dme-map-padding` | `40px` | 树外围留白 |
| `--dme-map-border` | `1px solid #dce5df` | 容器边框 |
| `--dme-map-radius` | `16px` | 容器圆角 |
| `--dme-gap-x` | `48px` | 父子间距 |
| `--dme-gap-y` | `20px` | 同级分支间距 |
| `--dme-node-max-width` | `230px` | 节点最大宽度 |
| `--dme-node-padding` | `10px 16px` | 普通节点内边距 |
| `--dme-node-border` | `1px solid #d3dfd6` | 普通节点边框 |
| `--dme-node-radius` | `12px` | 普通节点圆角 |
| `--dme-node-background` | `#fff` | 普通节点背景 |
| `--dme-node-color` | 基础文本色 | 普通节点文本 |
| `--dme-node-shadow` | `0 2px 5px #173b2910` | 节点阴影 |
| `--dme-root-background` | `#245846` | 根背景 |
| `--dme-root-color` | `#fff` | 根文本 |
| `--dme-root-border` | `1px solid transparent` | 根边框 |
| `--dme-root-radius` | `16px` | 根圆角 |
| `--dme-root-font-weight` | `600` | 根字重 |
| `--dme-root-padding` | `14px 20px` | 根内边距 |
| `--dme-link-color` | `#8fae9a` | 连线颜色 |
| `--dme-link-width` | `2px` | 连线宽度 |
| `--dme-error-padding` | `12px` | 错误留白 |
| `--dme-error-color` | `#a32e36` | 错误文本 |

变量值需符合所对应 CSS 属性的语法。结构布局规则仍由库维护。显式节点 style 优先于继承值；Theme API 生成同一套变量，插件变量见下表。

Dioxus 的 resize 观察覆盖节点、子容器、分支和画布，布局大小变化后重新读取实际矩形，连线不维护第二份 gap 常量。字体变大、padding 或间距变化可自动触发更新。对于未导致观察对象尺寸变化的外部布局移动，可通过增加 `measurement_revision` 主动重测；内建视口 transform 缩放保持本地几何，不需要对纯缩放重测；任意外部 transform/动画不承诺自动追踪。

## 交互与布局变量

| 变量 | 默认/用途 |
| --- | --- |
| `--dme-main-gap-x` | 继承 gap-x（普通 48px / compact 24px），主分支水平间距 |
| `--dme-main-gap-y` | 继承 gap-y（普通 20px / compact 10px），主分支垂直间距 |
| `--dme-node-cursor` | default，节点光标 |
| `--dme-node-hover-shadow` | 0 2px 8px #173b2928，悬停阴影 |
| `--dme-selected-outline` | 2px solid #388c69，选中外框 |
| `--dme-selected-offset` | 3px，选中外框偏移 |
| `--dme-focus-outline` | 2px solid #388c69，键盘焦点外框 |
| `--dme-focus-offset` | 3px，焦点偏移 |
| `--dme-collapse-gap` | 8px，按钮与节点外缘的间距，沿分支方向应用 |
| `--dme-collapse-size` | 24px，按钮宽高 |
| `--dme-collapse-border` | 1px solid #8fae9a，按钮边框 |
| `--dme-collapse-radius` | 50%，按钮圆角 |
| `--dme-collapse-background` | 继承画布背景（缺省 #f8faf7），遮住按钮下面的连线 |
| `--dme-collapse-color` | 继承 node-color/text-color（缺省 #245846），加减号颜色 |
| `--dme-editor-resize` | both，编辑区 resize |
| `--dme-editor-min-width` | 160px，编辑区最小宽度 |
| `--dme-editor-max-width` | 400px，编辑区最大宽度 |
| `--dme-editor-min-height` | 72px，编辑区最小高度 |
| `--dme-editor-padding` | 4px，编辑区留白 |
| `--dme-editor-border` | 1px solid #388c69，编辑区边框 |
| `--dme-editor-radius` | 4px，编辑区圆角 |
| `--dme-editor-outline` | none，编辑区外框 |
| `--dme-editor-background` | #fff，编辑背景 |
| `--dme-editor-color` | #263b36，编辑文字 |

compact 下 gap-x/gap-y 缺省为 24px/10px；宿主显式变量覆盖仍优先。`Theme::set_variable` 仅接受单个 CSS 值，拒绝分号/花括号；不提供双份 Rust 几何常量。显式 theme 比祖先继承优先，组件 style 可再覆盖 theme；省略 theme 时保留纯外部 CSS 定制路径。

## 折叠按钮定位

非根节点有子节点时显示折叠控件，位于节点朝向后代的一侧并沿轴居中：LEFT 在左，RIGHT 在右，SIDE 随各自主分支方向，DOWN 在下。按钮不会放在节点角上；根节点保留核心 API 折叠能力，但不显示分支按钮。编辑时隐藏按钮，避免聚焦冲突。

父子空隙至少容纳 `collapse-size + 2 × collapse-gap`，包括 compact 模式，防止放大控件或缩小 gap 后压住孩子节点。按钮默认背景继承画布，图标用 CSS 绘制、几何居中，不依赖字体中加减号的基线。

| 变量 | 默认/用途 |
| --- | --- |
| `--dme-collapse-shadow` | none，按钮阴影 |
| `--dme-collapse-hover-background` | 继承节点背景（缺省 #fff），悬停背景 |
| `--dme-collapse-hover-border` | 1px solid #388c69，悬停边框 |
| `--dme-collapse-icon-size` | 10px，加减号范围 |
| `--dme-collapse-icon-width` | 2px，加减号笔画宽度 |
| `--dme-collapse-icon-radius` | 1px，笔画圆角 |

旧原型的 `--dme-collapse-offset` / `--dme-collapse-bottom` 已由方向无关的 `--dme-collapse-gap` 替代。

## 插件变量

所有变量通过父容器继承，不在插件根节点重置。默认值见 `assets/mind-map.css`；同一个 `--dme-font-*` / `--dme-text-color` / `--dme-background` 同时影响地图、菜单、工具栏和大纲。显式 Theme 覆盖地图变量时，如需同步其他表面，请把变量放在这些组件的共同父元素。

| 表面 | 变量（均以 `--dme-` 开头） | 默认值 |
| --- | --- | --- |
| 工具栏 | `toolbar-gap`, `toolbar-padding`, `toolbar-background`, `toolbar-border`, `toolbar-radius` | 8px；12px；地图背景；1px solid #dce5df；12px |
| 命令按钮 | `command-padding`, `command-background`, `command-border`, `command-radius`, `command-hover-background`, `command-disabled-opacity` | 7px 12px；节点背景；1px solid #d3dfd6；8px；#e6f2e9；.45 |
| 菜单 | `menu-gap`, `menu-padding`, `menu-width`, `menu-min-width`, `menu-background`, `menu-border`, `menu-radius`, `menu-shadow` | 4px；8px；200px；170px；地图背景；1px solid #d3dfd6；12px；0 8px 30px #173b2930 |
| 大纲 | `outline-padding`, `outline-background`, `outline-border`, `outline-radius`, `outline-gap`, `outline-indent` | 16px；地图背景；1px solid #dce5df；12px；6px；20px |
| 大纲行 | `outline-row-padding`, `outline-row-radius`, `outline-row-height`, `outline-selected-background`, `outline-fold-size` | 6px；6px；38px；#dceee1；24px |
| 大纲动作 | `outline-action-padding`, `outline-action-size`, `breadcrumb-gap`, `breadcrumb-margin` | 2px 6px；12px；6px；12px |
| 大纲编辑 | 共用 `editor-color/background/border/radius/padding` | 与地图编辑一致 |
| 框选 | `marquee-background`, `marquee-border` | #388c6920；1px solid #388c69 |
| 拖动状态 | `drag-inset`, `drag-padding`, `drag-background`, `drag-color`, `drag-radius` | 12px；8px 12px；#245846；#fff；8px |
| 落点提示 | `drop-border`, `drop-radius`, `drop-background`, `drop-label-padding`, `drop-label-size`, `drop-point-size`, `drop-point-color` | 2px dashed #388c69；8px；#388c6918；3px 6px；12px；6px；#388c69 |

`--dme-outline-depth` 为每行内部计算的层级数，不是外部主题参数。交互命中阈值 5px/50px/12×scale 是与上游保持的行为契约；不是单独复制 CSS 间距。公开变量改变节点尺寸后，命中和连线均使用实际测量矩形。菜单使用 fixed 定位，放在所属地图 DOM 内继承样式；不建议宿主对整张地图设置外层 CSS transform（组件 viewport transform 不受影响）。

示例：

```css
.plugin-host {
  --dme-font-size: 18px;
  --dme-toolbar-background: #ede7f9;
  --dme-outline-background: #f1ecfa;
  --dme-outline-indent: 28px;
  --dme-command-radius: 14px;
  --dme-marquee-background: #8569c130;
  --dme-drop-border: 2px dashed #8569c1;
}
```

## 配图与备注（0.1.1）

| 变量 | 默认/用途 |
| --- | --- |
| `--dme-image-margin` | 0 auto 6px，配图外边距 |
| `--dme-image-radius` | 6px，配图圆角 |
| `--dme-image-missing-border` / `-color` / `-background` | 1px dashed #c9a2a6；#a32e36；transparent，图片无法显示时的占位 |
| `--dme-note-marker-size` / `--dme-note-marker-color` | 7px；#c08a2d，有备注的小圆点（大纲同色） |
| `--dme-image-badge-color` | #388c69，大纲中“有配图”的标记 |

`--dme-image-width` 由组件按节点配图宽度写在节点上，用来放宽节点最大宽度，不是外部主题参数。
