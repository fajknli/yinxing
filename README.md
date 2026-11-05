# 银杏 (Yinxing) — Wayland 原生音形码输入法

一个基于 "拼音首字母 + 字形结构 + 偏旁编码" 的输入法，使用纯 Rust 编写，运行在 Wayland (`input-method-v2`) 合成器上。

它结合了拼音的易学性与形码的精准度，无重度框架依赖，无后台遥测

## 设计原理 ISIR (Initial-Structure-Initial-Radical)

银杏采用定长编码，单字 3 键，词组 4-8 键。

### 1. 单字编码：`I + S + R` (3 键)

```
位置 1 (I): 拼音首字母
位置 2 (S): 字形结构码 (4 种)
位置 3 (R): 偏旁码 / 首笔画码
```

**示例：**
*   `河` = `h`(hé) + `s`(左右) + `i`(氵) = `hsi`
*   `想` = `x`(xiǎng) + `u`(上下) + `x`(心) = `xux`
*   `国` = `g`(guó) + `w`(全包围) + `k`(囗) = `gwk`
*   `这` = `z`(zhe) + `w`(半包围) + `z`(辶) = `zwz`
*   `人` = `r`(rén) + `y`(独体) + `p`(撇) = `ryp`

### 2. 词语编码：`ISIR` (极速词组)

ISIR (Initial-Structure-Initial-Radical)

采用"首字取音+结构，次字取音+偏旁，其余取音"的非对称设计。

```
二字词: I₁ + S₁ + I₂ + R₂        (你好 = nshn)
三字词: I₁ + S₁ + I₂ + R₂ + I₃   (计算机 = jsszj)
四字词: I₁ + S₁ + I₂ + R₂ + I₃ + I₄ (中华民族 = zyhhmz)
...支持最长 8 字词自动造词与编码。
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

> **特例：独体字 (S=y)**
> 当结构为独体时，位置 3 **不查偏旁**，而是取首笔画码：`h`(横)、`s`(竖)、`p`(撇)、`n`(点)、`z`(折)。

---

## 安装与部署

### 1. 环境依赖

*   Rust (stable)
*   Python 3 + `pypinyin` (用于生成码表)
*   Wayland 合成器 (需支持 `input-method-v2`, `virtual-keyboard-v1`, `wlr-layer-shell`，如 Hyprland, Sway, Niri)

### 2. 编译与运行

```bash
# 编译
cargo build --release

# 运行
./target/release/yinxing
```

### 3. 配置与热加载

配置文件位于 `~/.config/yinxing/config.toml`。可以自定义 UI 颜色、圆角、按键重复率、中英切换快捷键，甚至调整引擎的 DP 寻路深度 (`dp_beam_width`)。

修改配置后，无需重启输入法，只需在终端执行：
```bash
yinxing --reload
```
配置会通过 `SIGHUP` 信号瞬间热加载生效（字体除外）。

### 4. 生成专属词库

项目自带 Python 词库生成工具，可将基础汉字数据转化为音形码表。

```bash
# 1. 将 ids.txt, Unihan 数据, jieba_dict.txt 放入 data/ 目录
# 2. 运行生成脚本 (默认输出到 ~/.config/yinxing/dicts/)
python3 tools/gen_dict.py --all
```

## 许可证

本项目基于 AGPL-3.0-or-later 协议开源。
