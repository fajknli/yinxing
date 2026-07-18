# 银杏 (Yinxing) — Wayland 原生音形码输入法

一个基于 "拼音首字母 + 字形结构 + 偏旁编码" 的极速输入法，使用 Rust 编写，运行在 Wayland (`input-method-v2`) 合成器上。

它结合了拼音的易学性与形码的精准度，旨在提供**零延迟、低记忆负担、极少重码**的极致打字体验。

## 设计原理

银杏采用定长编码，彻底消除编码歧义。单字 3 键，词语 4-6 键。

### 1. 单字编码：`P + S + R` (3 键)

```
位置 1 (P): 拼音首字母
位置 2 (S): 字形结构码 (4 种)
位置 3 (R): 偏旁码 / 首笔画码
```

**示例：**
*   `河` = `h`(hé) + `s`(左右) + `i`(氵) = `hsi`
*   `想` = `x`(xiǎng) + `u`(上下) + `x`(心) = `xux`
*   `国` = `g`(guó) + `w`(全包围) + `w`(囗) = `gww`
*   `人` = `r`(rén) + `y`(独体) + `p`(撇) = `ryp`

### 2. 词语编码：`ISIR` (极速词组)

采用"首字取音+结构，尾字取音+偏旁"的非对称设计，打词时节奏感极强，且码空间巨大。

```
二字词: P₁ + S₁ + P₂ + R₂        (你好 = nshn)
三字词: P₁ + S₁ + P₂ + R₂ + P₃   (计算机 = jsszj)
四字词: P₁ + S₁ + P₂ + R₂ + P₃ + P₄ (中华民族 = zyhhmz)
```

---

## 编码映射表

### 位置 2 — 结构码 (仅 4 种)

| 键 | 结构 | 占比 | 说明 |
|---|---|---|---|
| **s** | 左右 | 63.4% | 左右、左中右结构 |
| **u** | 上下 | 24.2% | 上下、上中下结构 |
| **w** | 包围 | 9.0% | 全包围、半包围（走之底等） |
| **y** | 独体 | 3.3% | 不可拆分的整体字 |

### 位置 3 — 偏旁码

do what u want to

> **特例：独体字 (S=y)**
> 当结构为独体时，位置 3 **不查偏旁**，而是取首笔画码：`h`(横)、`s`(竖)、`p`(撇)、`n`(点)、`z`(折)。严格实现"形归形，音归音"。

---

## 引擎特性

*   **Viterbi DP 解码**：支持长句连打，自动寻找最优分词路径。
*   **残余输入保留**：选词上屏后，未匹配的编码自动留在输入框继续匹配。
*   **零拷贝字典查询**：Rust 引擎使用 `Rc<str>` 与切片引用，每次按键零堆内存分配。
*   **WPV 权重排序**：结合 Jieba 词频与用户学习词频，高频词永远排第一。
*   **原子化词库写入**：用户词库定时落盘，采用 `tmp + rename` 机制，断电不损坏。

---

## 操作快捷键

| 按键 | 功能 |
|---|---|
| `Ctrl+Space` | 切换中/英文 |
| `Shift` (单击) | 临时切换英文 / 切回中文 |
| `字母键` | 输入编码 (中文模式) |
| `空格` | 选第 1 个候选词 |
| `1-9` | 选第 N 个候选词 |
| `Tab` / `=` | 下一页 |
| `-` | 上一页 |
| `Backspace` | 删除最后一个编码 |
| `Esc` | 清空输入框 |
| `Enter` | 提交原始英文字母 (不选候选) |
| `,` `.` `/` `;` | 自动输出中文标点 `，` `。` `？` `；` |

---

## 安装与部署

### 1. 环境依赖

*   Rust (stable)
*   Python 3 + `pypinyin` (用于生成码表)
*   Wayland 合成器 (需支持 `input-method-v2`, `virtual-keyboard-v1`, `wlr-layer-shell`，如 Hyprland, Sway, Niri)

### 2. 生成码表

```bash
# 安装 Python 依赖
pip install pypinyin

# 下载字典数据源
wget https://raw.githubusercontent.com/cjkvi/cjkvi-ids/master/ids.txt
wget https://raw.githubusercontent.com/fxsjy/jieba/master/jieba/dict.txt -O dicts/jieba_dict.txt
# 解压 Unihan.zip 到 unihan/ 目录

# 生成全部字典 (包含单字、词语、统计报告，并自动部署到 ~/.config/yinxing/)
python3 tools/gen_dict.py --all
```

### 3. 编译运行

```bash
cargo build --release
./target/release/yinxing
```

### 4. 开机自启 (以 Hyprland 为例)

在 `~/.config/hyprland.conf` 中添加：
```ini
exec-once = /path/to/yinxing
```

---

## 配置文件

配置文件位于 `~/.config/yinxing/config.toml`：

```toml
[appearance]
font = ""             # 留空则使用系统默认 Sans字体
font_size = 16.0
bg = 0xE61a1a1a       # ARGB 格式
fg = 0xFFe0e0e0
selected_bg = 0xFF3a6ea5
selected_fg = 0xFFffffff
hpad = 12.0           # 水平内边距
vpad = 4.0            # 垂直内边距
gap = 6.0             # 候选词间距
max_items = 9         # 每页显示候选数
anchor = "bottom_center" # 候选框位置

[dicts]
chars = "dicts/chars.txt"
words = "dicts/words.txt"
user = "dicts/user.txt"  # 用户学习词库
```

---

## 码表生成工具详解

`tools/gen_dict.py`

1.  **结构判定**：优先查人工覆盖表 (`STRUCTURE_OVERRIDES`)，其次解析 IDS 表达式，无法拆分则归为独体 `y`。
2.  **偏旁提取**：优先递归拆解 IDS 寻找**视觉首部件**；若失败，回退到 Unihan **康熙部首**；若再失败，归入杂类 `z`。
3.  **独体字特赦**：如果是独体字 (`y`)，抛弃所有偏旁逻辑，强制使用**首笔画**作为第三码。
4.  **词频缩放**：将 Jieba 的绝对词频通过对数缩放 (`math.log`) 压缩到 10-100000 的平滑区间，防止高频词权重溢出，同时保证单字与词语在 DP 解码中公平竞争。

### 字典质量指标 (GB2312 6763 字)

```
平均重码: 5.9 字/编码
无重码率: 24.4%
最大重码桶: 42 字 (yua: 芽宴羊养药夜衣...)
偏旁杂类(z)占比: 7.1% (彻底受控)
```

## 许可证

MIT
