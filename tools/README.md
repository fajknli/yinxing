# 下载 jieba 词库
wget https://raw.githubusercontent.com/fxsjy/jieba/master/jieba/dict.txt -O dicts/jieba_dict.txt

# 生成全部码表
python3 tools/gen_dict.py --all

# 复制到配置目录
cp dicts/chars.txt ~/.config/yinxing/dicts/
cp dicts/words.txt ~/.config/yinxing/dicts/


----

## gen_dict.py 工作流程

```
步骤 1: 从 GB2312 编码范围枚举汉字
         0xB0-0xD7 → 3755 个一级字
         不需要任何文件，纯数学枚举

步骤 2: 查 ids.txt（88939行，~30000个唯一汉字）
         获取每个字的拆分表达式
         例: "河" → "⿰氵可"
         用途: 判断结构码（左右/上下/包围）+ 提取偏旁

步骤 3: 查 Unihan_IRGSources.txt（102550个汉字的元数据）
         获取康熙部首编号 + 总笔画数
         例: "河" → 部首=85(水), 笔画=8
         用途: 当 ids.txt 提取偏旁失败时，用部首编号回退

步骤 4: 查 pypinyin 库（实时计算，不需要文件）
         获取每个字的拼音首字母
         例: "河" → "hé" → "h"

步骤 5: 查 jieba_dict.txt（31万词语+频率）
         获取单字频率 + 词语编码
         例: "河" 频率=29996, "黄河" 编码=huhi
```

## 三个文件的角色

```
ids.txt                  = 字的结构（怎么拆）
Unihan_IRGSources.txt    = 字的属性（部首、笔画）
jieba_dict.txt           = 字词的频率和词语列表
```

**生成的码表只有 chars.txt（单字）和 words.txt（词语）。** 三个源文件只读不写。
