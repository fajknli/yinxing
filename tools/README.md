# 音形码词库生成工具

纯 Python 引擎，从汉字结构 (IDS)、部首 (Unihan)、字频 (Jieba/SUBTLEX-CH) 自动生成音形码单字码表与词语码表。

## 依赖

```bash
pip install pypinyin openpyxl
```

## 数据准备

将以下文件放入 `data/` 目录：

| 文件 | 来源 | 必需 |
|------|------|------|
| `ids.txt` | CHISE IDS 拆分表 | 是 |
| `unihan/Unihan_IRGSources.txt` | Unihan 汉字属性库 | 是 |
| `jieba_dict.txt` | 结巴词库 | 是 |
| `SUBTLEX-CH-WF.xlsx` | SUBTLEX-CH 词频库 | 否（推荐） |

下载：

```bash
cd data/

# IDS 汉字结构拆分表
wget -O ids.txt https://raw.githubusercontent.com/cjkvi/cjkvi-ids/master/ids.txt

# Unihan 汉字属性库
wget https://www.unicode.org/Public/UCD/latest/ucd/Unihan.zip
unzip Unihan.zip -d unihan/

# 结巴词库
wget -O jieba_dict.txt https://raw.githubusercontent.com/fxsjy/jieba/master/jieba/dict.txt

# SUBTLEX-CH 词频库（可选）
wget https://www.ugent.be/pp/experimentele-psychologie/en/research/documents/subtlexch/subtlexchwf.zip
unzip subtlexchwf.zip
mv SUBTLEX-CH-WF.xlsx SUBTLEX-CH-WF.xlsx   # 已是正确文件名，无需改名
```

## 用法

```bash
# 完整生成（默认：单字 + 词语 + 报告 + 部署）
python3 tools/gen_dict.py

# 分步执行
python3 tools/gen_dict.py --chars            # 仅单字码表
python3 tools/gen_dict.py --words            # 仅词语码表
python3 tools/gen_dict.py --report           # 仅统计报告

# 词语数据源切换
python3 tools/gen_dict.py --all --words-source subtlex   # SUBTLEX-CH（默认）
python3 tools/gen_dict.py --all --words-source jieba     # 结巴词库
python3 tools/gen_dict.py --all --words-source none      # 跳过词语

# 诊断与分析
python3 tools/gen_dict.py --verify 想       # 校验单字编码
python3 tools/gen_dict.py --compare         # IDS vs Unihan 部首一致性
python3 tools/gen_dict.py --check-first     # IDS 第一部件匹配检查
python3 tools/gen_dict.py --list-no-ids     # 导出无结构运算符字符
python3 tools/gen_dict.py --inspect         # IDS 数据库深度结构分析
python3 tools/gen_dict.py --report-radicals # 偏旁码频率分布
python3 tools/gen_dict.py --benchmark       # 词频阈值重码率基准
```

生成完成后自动部署到 `~/.config/yinxing/dicts/`。

## CLI 参数速查

| 参数 | 说明 |
|------|------|
| `--all` | 执行完整流程（默认） |
| `--chars` | 生成 `chars.txt` |
| `--words` | 生成 `words.txt` |
| `--report` | 生成 `char_report.txt` |
| `--verify <字>` | 输出单字编码详细拆解 |
| `--compare` | IDS 部件 vs Unihan 部首对照 |
| `--check-first` | IDS 第一部件偏旁表匹配率 |
| `--list-no-ids` | 导出无结构运算符字符清单 |
| `--inspect` | IDS 数据库结构全景分析 |
| `--report-radicals` | 偏旁码字母分布均匀度 |
| `--benchmark` | 不同词频阈值下的重码率 |
| `--words-source` | 词语源：`subtlex` / `jieba` / `none` |
| `--min-freq <N>` | jieba 源最低词频（默认 200） |

## 编码规则

### 单字码（3 键）

```
编码 = 拼音首字母 (I) + 结构码 (S) + 偏旁/首笔画码 (R)
```

**拼音首字母 (I)**

pypinyin 取首字母，`v` 归入 `u`。多音字低频读音舍弃（字频 < 100000 时仅保留第一个读音）。可通过 `MULTI_PINYIN_OVERRIDE` 硬编码覆盖。

**结构码 (S)**

解析 IDS 表达式的根操作符：

| 码 | 结构 | IDS 操作符 |
|----|------|-----------|
| `s` | 左右 | `⿰` `⿲` |
| `u` | 上下 | `⿱` `⿳` |
| `w` | 包围 | `⿴` `⿵` `⿶` `⿷` `⿸` `⿹` `⿺` |
| `y` | 独体/交叠/无结构 | `⿻` 或无操作符 |

无 IDS 数据或无法解析的默认为 `y`。可通过 `STRUCTURE_OVERRIDES` 覆盖。

**偏旁/首笔画码 (R)**

- **合体字**：递归遍历 IDS 表达式，取第一个命中 `RADICAL_CHAR_TO_CODE` 映射表的部件，映射到 `a-z`。
- **独体字 (`y`)**：查 `FIRST_STROKE_MAP` 取首笔画码：

| 码 | 笔画 | 示例 |
|----|------|------|
| `h` | 横 | 一 三 大 王 木 |
| `n` | 点/捺 | 丶 为 主 心 立 |
| `p` | 撇 | 丿 人 几 月 竹 |
| `s` | 竖 | 丨 中 口 日 山 |
| `z` | 折 | 乙 刀 飞 了 弓 |

可通过 `CHAR_RADICAL_OVERRIDES` 覆盖偏旁码。

**示例**

```
想 → x(想→x) + u(⿱ 上下) + o(目→o) = "xuo"
明 → m(明→m) + s(⿰ 左右) + o(日→o) = "mso"
国 → g(国→g) + w(⿴ 包围) + e(囗→e)... 实际取玉→y = "gwy"
```

### 词语码（N 键，2-8 字词）

```
编码 = 首字(IS) + 次字(IR) + 第3字(I) + ... + 第N字(I)
```

- 首字取拼音首字母 + 结构码
- 次字取拼音首字母 + 偏旁/首笔画码
- 第三字起仅取拼音首字母

```
今天 → 今(j+上下u) + 天(t+横h) = "juth"
中国 → 中(s+竖s) + 国(g+包围w) = "ssgw"
```

## 权重计算

权重范围 `10 ~ 100000`，用于输入法候选排序。

**单字**：有 jieba 字频时取对数归一化 `int(10 + norm * 990)`；无字频时按笔画回退 `max(10, 200 - strokes * 5)`。

**词语**：`int(log(freq+1) * 5000 * multiplier)`，词长乘数 2 字=2.6、3 字=3.6、4 字=4.5。

## 输出文件

```
dicts/
├── chars.txt                 # 单字码表：字 \t 编码 \t 权重
├── words.txt                 # 词语码表：词 \t 编码 \t 权重
└── reports/
    ├── char_report.txt       # 结构/偏旁分布 + 重码桶分析 + 问题字清单
    ├── compare_report.txt    # IDS 部件 vs Unihan 部首对照
    ├── ids_first_check.txt   # IDS 第一部件匹配检查
    └── no_ids_list.txt       # 无结构运算符字符清单
dicts/radical_report.txt      # 偏旁码频率分布（--report-radicals 生成）
```

## 数据来源

- [cjkvi-ids](https://github.com/cjkvi/cjkvi-ids) — IDS 汉字结构拆分
- [Unihan](https://www.unicode.org/charts/unihan.html) — 康熙部首与笔画
- [jieba](https://github.com/fxsjy/jieba) — 字词频率
- [SUBTLEX-CH](https://www.ugent.be/pp/experimentele-psychologie/en/research/documents/subtlexch) — 中文字词频率（研究级）
