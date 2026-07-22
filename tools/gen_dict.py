# yinxing - 音形码输入法
# Copyright (C) 2026  fajknli
# Licensed under the GNU AGPL v3. See LICENSE for details.

#!/usr/bin/env python3
"""音形码词库生成工具"""

import sys
import os
import argparse
import pypinyin
import math
import shutil
from collections import Counter


def is_cjk(c):
    return '\u4e00' <= c <= '\u9fff'


def is_ext_cjk(c):
    return '\u3400' <= c <= '\u4dbf' or '\U00020000' <= c <= '\U0002a6df'


ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)

IDS_FILE    = os.path.join(ROOT, "data", "ids.txt")
UNIHAN_FILE = os.path.join(ROOT, "data", "unihan/Unihan_IRGSources.txt")
JIEBA_FILE  = os.path.join(ROOT, "data", "jieba_dict.txt")
CHARS_FILE  = os.path.join(ROOT, "dicts/chars.txt")
WORDS_FILE  = os.path.join(ROOT, "dicts/words.txt")
REPORT_FILE = os.path.join(ROOT, "dicts/reports/char_report.txt")
COMPARE_FILE = os.path.join(ROOT, "dicts/reports/compare_report.txt")
IDS_FIRST_CHECK_FILE = os.path.join(ROOT, "dicts/reports/ids_first_check.txt")
NO_IDS_FILE = os.path.join(ROOT, "dicts/reports/no_ids_list.txt")
SUBTLEX_FILE = os.path.join(ROOT, "data", "SUBTLEX-CH-WF.xlsx")

MULTI_PINYIN_OVERRIDE = {
    '行': 'x', '重': 'c', '长': 'c', '乐': 'l', '都': 'd', '还': 'h',
    '觉': 'j', '得': 'd', '着': 'z', '了': 'l', '看': 'k', '好': 'h',
    '发': 'f', '打': 'd', '当': 'd', '作': 'z', '种': 'z', '传': 'c',
    '干': 'g', '分': 'f', '省': 's', '相': 'x', '难': 'n',
    '调': 't', '单': 'd', '教': 'j', '便': 'b', '量': 'l',
}

IDS_OPERATORS = set('⿰⿱⿲⿳⿴⿵⿶⿷⿸⿹⿺⿻')

STRUCTURE_OVERRIDES = {
    '幽': 'w', '坐': 'u', '果': 'u',
}

# =====================================================================
#  多音字 / 结构 覆盖表
# =====================================================================

IDS_TO_STRUCTURE = {
    '⿰': 's', '⿲': 's',
    '⿱': 'u', '⿳': 'u',
    '⿴': 'w', '⿵': 'w', '⿶': 'w', '⿷': 'w',
    '⿸': 'w', '⿹': 'w', '⿺': 'w',
    '⿻': 'y',
}

# =====================================================================
#  康熙部首表（214 个全覆盖）
# =====================================================================

RADICAL_NUM_TO_CHAR = {
    1: '一', 2: '丨', 3: '丶', 4: '丿', 5: '乙', 6: '亅', 7: '二',
    8: '亠', 9: '人', 10: '儿', 11: '入', 12: '八', 13: '冂', 14: '冖',
    15: '冫', 16: '几', 17: '凵', 18: '刀', 19: '力', 20: '勹', 21: '匕',
    22: '匚', 23: '匸', 24: '十', 25: '卜', 26: '卩', 27: '厂', 28: '厶',
    29: '又', 30: '口', 31: '囗', 32: '土', 33: '士', 34: '夂', 35: '夊',
    36: '夕', 37: '大', 38: '女', 39: '子', 40: '宀', 41: '寸', 42: '小',
    43: '尢', 44: '尸', 45: '屮', 46: '山', 47: '巛', 48: '工', 49: '己',
    50: '巾', 51: '干', 52: '幺', 53: '广', 54: '廴', 55: '廾', 56: '弋',
    57: '弓', 58: '彐', 59: '彡', 60: '彳', 61: '心', 62: '戈', 63: '户',
    64: '手', 65: '支', 66: '攴', 67: '文', 68: '斗', 69: '斤', 70: '方',
    71: '无', 72: '日', 73: '曰', 74: '月', 75: '木', 76: '欠', 77: '止',
    78: '歹', 79: '殳', 80: '毋', 81: '比', 82: '毛', 83: '氏', 84: '气',
    85: '水', 86: '火', 87: '爪', 88: '父', 89: '爻', 90: '爿', 91: '片',
    92: '牙', 93: '牛', 94: '犬', 95: '玄', 96: '玉', 97: '瓜', 98: '瓦',
    99: '甘', 100: '生', 101: '用', 102: '田', 103: '疋', 104: '疒', 105: '癶',
    106: '白', 107: '皮', 108: '皿', 109: '目', 110: '矛', 111: '矢', 112: '石',
    113: '示', 114: '禸', 115: '禾', 116: '穴', 117: '立', 118: '竹', 119: '米',
    120: '糸', 121: '缶', 122: '网', 123: '羊', 124: '羽', 125: '老', 126: '而',
    127: '耒', 128: '耳', 129: '聿', 130: '肉', 131: '臣', 132: '自', 133: '至',
    134: '臼', 135: '舌', 136: '舛', 137: '舟', 138: '艮', 139: '色', 140: '艸',
    141: '虍', 142: '虫', 143: '血', 144: '行', 145: '衣', 146: '襾', 147: '見',
    148: '角', 149: '言', 150: '谷', 151: '豆', 152: '豕', 153: '豸', 154: '貝',
    155: '赤', 156: '走', 157: '足', 158: '身', 159: '車', 160: '辛', 161: '辰',
    162: '辵', 163: '邑', 164: '酉', 165: '釆', 166: '里', 167: '金', 168: '長',
    169: '門', 170: '阜', 171: '隶', 172: '隹', 173: '雨', 174: '靑', 175: '非',
    176: '面', 177: '革', 178: '韋', 179: '韭', 180: '音', 181: '頁', 182: '風',
    183: '飛', 184: '食', 185: '首', 186: '香', 187: '馬', 188: '骨', 189: '高',
    190: '髟', 191: '鬥', 192: '鬯', 193: '鬲', 194: '鬼', 195: '魚', 196: '鳥',
    197: '鹵', 198: '鹿', 199: '麥', 200: '麻', 201: '黃', 202: '黍', 203: '黑',
    204: '黹', 205: '黽', 206: '鼎', 207: '鼓', 208: '鼠', 209: '鼻', 210: '齊',
    211: '齒', 212: '龍', 213: '龜', 214: '龠',
}

RADICAL_CHAR_TO_CODE = {
    # ═══ 映射到 a 的部件 ═══
    '艸': 'a', '艹': 'a', '𫇦': 'a', '䒑': 'a', '亠': 'a', '⺌': 'a', '𦭝': 'a', '龸': 'a', '𭕄': 'a', '宀': 'a',

    # ═══ 映射到 b 的部件 ═══
    '髟': 'b', '貝': 'b', '贝': 'b', '白': 'b', '比': 'b', '鼻': 'b', '卜': 'b', '八': 'b', '匕': 'b', '丷': 'b',
    '癶': 'b', '⺊': 'b', '𫧇': 'b', '半': 'b', '北': 'b', '不': 'b', '包': 'b', '卑': 'b', '㡀': 'b',

    # ═══ 映射到 c 的部件 ═══
    '車': 'c', '车': 'c', '辰': 'c', '齒': 'c', '齿': 'c', '虫': 'c', '寸': 'c', '川': 'c', '厂': 'c', '屮': 'c',
    '臣': 'c', '赤': 'c', '長': 'c', '鬯': 'c', '舛': 'c', '春': 'c', '丞': 'c', '尺': 'c', '次': 'c', '𠦝': 'c',
    '敕': 'c', '䖵': 'c',

    # ═══ 映射到 d 的部件 ═══
    '大': 'd', '刀': 'd', '刂': 'd', '斗': 'd', '豆': 'd', '歹': 'd', '扌': 'd', '鬥': 'd', '鼎': 'd', '单': 'd',
    '毒': 'd', '旦': 'd', '翟': 'd',

    # ═══ 映射到 e 的部件 ═══
    '阝': 'e', '耳': 'e', '儿': 'e', '二': 'e', '而': 'e', '阜': 'e', '邑': 'e', '〢': 'e',

    # ═══ 映射到 f 的部件 ═══
    '方': 'f', '风': 'f', '風': 'f', '父': 'f', '非': 'f', '飛': 'f', '丰': 'f', '缶': 'f', '反': 'f', '夆': 'f',
    '孚': 'f', '负': 'f', '分': 'f', '凡': 'f',

    # ═══ 映射到 g 的部件 ═══
    '干': 'g', '甘': 'g', '戈': 'g', '革': 'g', '骨': 'g', '鬼': 'g', '弓': 'g', '广': 'g', '瓜': 'g', '艮': 'g',
    '谷': 'g', '高': 'g', '果': 'g', '鼓': 'g', '龜': 'g', '龟': 'g', '廾': 'g', '工': 'g', '共': 'g', '羔': 'g',
    '光': 'g', '古': 'g', '𢦏': 'g',

    # ═══ 映射到 h 的部件 ═══
    '火': 'h', '灬': 'h', '禾': 'h', '黑': 'h', '黄': 'h', '黃': 'h', '十': 'h', '户': 'h', '戶': 'h', '虍': 'h',
    '鬲': 'h', '一': 'h', '冖': 'h', '好': 'h', '虎': 'h', '丆': 'h',

    # ═══ 映射到 i 的部件 ═══
    '水': 'i', '氵': 'i', '冫': 'i',

    # ═══ 映射到 j 的部件 ═══
    '几': 'j', '己': 'j', '巾': 'j', '斤': 'j', '金': 'j', '钅': 'j', '釒': 'j', '角': 'j', '臼': 'j', '见': 'j',
    '見': 'j', '韭': 'j', '彐': 'j', '亅': 'j', '卩': 'j', '句': 'j', '京': 'j', '加': 'j', '卪': 'j', '旡': 'j',
    '既': 'j', '甲': 'j', '军': 'j',

    # ═══ 映射到 k 的部件 ═══
    '口': 'k', '囗': 'k', '凵': 'k', '冂': 'k', '匚': 'k', '匸': 'k', '⺆': 'k', '可': 'k', '克': 'k', '夸': 'k',
    '𠮛': 'k', '叩': 'k', '咠': 'k', '囬': 'k', '𠱠': 'k',

    # ═══ 映射到 l 的部件 ═══
    '立': 'l', '里': 'l', '力': 'l', '老': 'l', '耂': 'l', '耒': 'l', '鹿': 'l', '龙': 'l', '龍': 'l', '卤': 'l',
    '鹵': 'l', '隶': 'l', '卵': 'l', '林': 'l', '吕': 'l', '㐬': 'l', '劦': 'l', '寽': 'l', '粦': 'l',

    # ═══ 映射到 m 的部件 ═══
    '门': 'm', '門': 'm', '木': 'm', '米': 'm', '毛': 'm', '马': 'm', '馬': 'm', '母': 'm', '皿': 'm', '面': 'm',
    '麦': 'm', '麥': 'm', '麻': 'm', '矛': 'm', '黾': 'm', '黽': 'm', '莫': 'm', '美': 'm', '朩': 'm', '朿': 'm',
    '𥃭': 'm', '冃': 'm',

    # ═══ 映射到 n 的部件 ═══
    '女': 'n', '牛': 'n', '鸟': 'n', '鳥': 'n', '丶': 'n', '礻': 'n', '衤': 'n', '疒': 'n', '㇀': 'n', '⺀': 'n',

    # ═══ 映射到 o 的部件 ═══
    '日': 'o', '目': 'o', '曰': 'o', '月': 'o',

    # ═══ 映射到 p 的部件 ═══
    '皮': 'p', '攴': 'p', '攵': 'p', '夂': 'p', '夊': 'p', '片': 'p', '爿': 'p', '丬': 'p', '疋': 'p', '矢': 'p',
    '釆': 'p', '夕': 'p', '爫': 'p', '𤴔': 'p', '𠂊': 'n', '丿': 'p', '彡': 'p', '勹': 'p', '𠂆': 'p', '𠂑': 'p',
    '𠂎': 'p', '频': 'p', '厃': 'p', '𠂋': 'p',

    # ═══ 映射到 q 的部件 ═══
    '犬': 'q', '犭': 'q', '欠': 'q', '气': 'q', '齐': 'q', '齊': 'q', '青': 'q', '靑': 'q', '且': 'q', '取': 'q',

    # ═══ 映射到 r 的部件 ═══
    '人': 'r', '亻': 'r', '肉': 'r', '禸': 'r', '入': 'r', '彳': 'r', '亽': 'r', '䏍': 'r', '亼': 'r', '仌': 'r',
    '𠇍': 'r', '𡗗': 'r', '龹': 'r',

    # ═══ 映射到 s 的部件 ═══
    '手': 's', '石': 's', '糹': 's', '纟': 's', '食': 's', '饣': 's', '飠': 's', '示': 's', '舌': 's', '身': 's',
    '山': 's', '生': 's', '尸': 's', '鼠': 's', '豕': 's', '氏': 's', '殳': 's', '色': 's', '首': 's', '黍': 's',
    '丨': 's', '申': 's', '束': 's', '舍': 's', '壴': 's', '杀': 's', '式': 's', '思': 's', '蜀': 's', '隋': 's',
    '𠕁': 's',

    # ═══ 映射到 t 的部件 ═══
    '土': 't', '士': 't', '田': 't', '天': 't', '龶': 't', '厽': 't', '𠫓': 't', '头': 't',

    # ═══ 映射到 w 的部件 ═══
    '王': 'w', '瓦': 'w', '韦': 'w', '韋': 'w', '网': 'w', '罒': 'w', '尢': 'w', '文': 'w', '无': 'w', '毋': 'w',
    '亡': 'w', '戊': 'w', '兀': 'w', '丸': 'w', '玨': 'w',

    # ═══ 映射到 x 的部件 ═══
    '心': 'x', '忄': 'x', '穴': 'x', '血': 'x', '辛': 'x', '行': 'x', '香': 'x', '玄': 'x', '小': 'x', '覀': 'x',
    '西': 'x', '糸': 'x', '兄': 'x', '㐅': 'x', '㣺': 'x', '系': 'x', '习': 'x',

    # ═══ 映射到 y 的部件 ═══
    '衣': 'y', '言': 'y', '讠': 'y', '訁': 'y', '酉': 'y', '羊': 'y', '用': 'y', '幺': 'y', '又': 'y', '弋': 'y',
    '聿': 'y', '廴': 'y', '爻': 'y', '牙': 'y', '襾': 'y', '龠': 'y', '玉': 'y', '雨': 'y', '羽': 'y', '鱼': 'y',
    '魚': 'y', '页': 'y', '頁': 'y', '音': 'y', '云': 'y', '予': 'y', '尤': 'y', '𧘇': 'y', '𠃓': 'y', '異': 'y',
    '叒': 'y', '冝': 'y', '厌': 'y', '业': 'y', '益': 'y', '壹': 'y', '𦍌': 'y', '𦣞': 'y', '𧰨': 'y', '䍃': 'y',

    # ═══ 映射到 z 的部件 ═══
    '走': 'z', '辶': 'z', '辵': 'z', '竹': 'z', '⺮': 'z', '舟': 'z', '子': 'z', '自': 'z', '至': 'z', '足': 'z',
    '𧾷': 'z', '隹': 'z', '止': 'z', '爪': 'z', '黹': 'z', '支': 'z', '豸': 'z', '乙': 'z', '周': 'z', '厶': 'z',
    '直': 'z', '兹': 'z', '𠃌': 'z', '㇇': 'z', '⺄': 'z', '㔾': 'z', '乛': 'z', '乚': 'z', '彑': 'z', '卂': 'z',
    '龴': 'z', '亇': 'z', '𠄐': 'z', '𡿨': 'z', '巜': 'z', '巛': 'z', '责': 'z', '占': 'z', '展': 'z', '正': 'z',
    '只': 'z', '恣': 'z',
}

CHAR_RADICAL_OVERRIDES = {
    '师': 'e',
    '甚': 'q',
}

# =====================================================================
#  独体字首笔画映射
# =====================================================================

FIRST_STROKE_MAP = {
    # ═══ 横起笔 (h) ═══
    '一': 'h', '七': 'h', '万': 'h', '丈': 'h', '三': 'h', '上': 'h',
    '下': 'h', '不': 'h', '丐': 'h', '专': 'h', '世': 'h', '东': 'h',
    '丰': 'h', '事': 'h', '二': 'h', '于': 'h', '五': 'h', '井': 'h',
    '其': 'h', '再': 'h', '凸': 'h', '出': 'h', '北': 'h', '十': 'h',
    '卅': 'h', '南': 'h', '厂': 'h', '右': 'h', '吏': 'h', '土': 'h',
    '士': 'h', '大': 'h', '天': 'h', '太': 'h', '夫': 'h', '夷': 'h',
    '夹': 'h', '子': 'h', '寸': 'h', '尢': 'h',
    '屯': 'h', '工': 'h', '左': 'h', '巫': 'h', '干': 'h', '平': 'h',
    '廾': 'h', '廿': 'h', '开': 'h', '戈': 'h', '戊': 'h', '戋': 'h',
    '成': 'h', '或': 'h', '才': 'h', '无': 'h', '更': 'h', '木': 'h',
    '未': 'h', '末': 'h', '本': 'h', '束': 'h', '来': 'h', '柬': 'h',
    '水': 'h', '爽': 'h', '牙': 'h', '犬': 'h', '王': 'h',
    '瓦': 'h', '甘': 'h', '示': 'h', '耒': 'h', '耳': 'h', '西': 'h',
    '豆': 'h', '车': 'h', '酉': 'h', '隶': 'h', '雨': 'h', '面': 'h',
    '革': 'h', '韦': 'h', '页': 'h', '马': 'h', '麦': 'h', '黄': 'h',
    '龙': 'h', '巨': 'h', '臣': 'h',

    # ═══ 点起笔 (n) ═══
    '丶': 'n', '为': 'n', '主': 'n', '之': 'n', '六': 'n', '农': 'n',
    '头': 'n', '州': 'n', '广': 'n', '心': 'n', '必': 'n', '文': 'n',
    '斗': 'n', '方': 'n', '术': 'n', '永': 'n', '火': 'n', '立': 'n',
    '米': 'n', '言': 'n', '齐': 'n',

    # ═══ 撇起笔 (p) ═══
    '丘': 'p', '个': 'p', '丸': 'p', '丹': 'p', '丿': 'p', '久': 'p',
    '么': 'p', '乍': 'p', '乎': 'p', '乐': 'p', '乖': 'p', '乘': 'p',
    '九': 'p', '人': 'p', '入': 'p', '八': 'p', '几': 'p', '勹': 'p',
    '匆': 'p', '升': 'p', '及': 'p', '向': 'p',
    '失': 'p', '女': 'p', '川': 'p', '年': 'p', '彳': 'p', '我': 'p',
    '手': 'p', '攵': 'p', '斤': 'p', '斥': 'p', '月': 'p', '朱': 'p',
    '欠': 'p', '母': 'p', '氏': 'p', '爪': 'p', '片': 'p', '牛': 'p',
    '犭': 'p', '瓜': 'p', '生': 'p', '用': 'p', '白': 'p', '禾': 'p',
    '秉': 'p', '竹': 'p', '自': 'p', '臼': 'p', '舟': 'p', '血': 'p',
    '角': 'p', '谷': 'p', '豸': 'p', '身': 'p', '重': 'p', '长': 'p',
    '隹': 'p', '风': 'p', '鱼': 'p', '鸟': 'p', '鼠': 'p', '鼻': 'p',
    '龟': 'p', '缶': 'p', '册': 'p', '熏': 'p', '禹': 'p', '兆': 'p',
    '臾': 'p',

    # ═══ 竖起笔 (s) ═══
    '且': 's', '丨': 's', '中': 's', '串': 's', '内': 's', '冉': 's',
    '凹': 's', '卜': 's', '口': 's', '史': 's', '囗': 's', '四': 's',
    '央': 's', '屮': 's', '山': 's', '巾': 's', '日': 's', '曰': 's',
    '曲': 's', '曳': 's', '果': 's', '止': 's', '田': 's', '由': 's',
    '甲': 's', '申': 's', '电': 's', '皿': 's', '目': 's', '肉': 's',
    '虫': 's', '见': 's', '贝': 's', '黑': 's', '齿': 's', '非': 's',
    '禺': 's', '豳': 's', '门': 's',

    # ═══ 折起笔 (z) ═══
    '丑': 'z', '乃': 'z', '乜': 'z', '也': 'z', '习': 'z', '乡': 'z',
    '书': 'z', '了': 'z', '以': 'z', '刀': 'z', '力': 'z', '办': 'z',
    '卩': 'z', '厶': 'z', '又': 'z', '发': 'z', '孑': 'z', '孓': 'z',
    '尸': 'z', '尹': 'z', '尺': 'z', '己': 'z', '已': 'z', '巳': 'z',
    '巴': 'z', '廴': 'z', '弓': 'z', '弗': 'z', '彐': 'z', '承': 'z',
    '毋': 'z', '民': 'z', '疋': 'z', '皮': 'z', '纟': 'z', '聿': 'z',
    '肃': 'z', '艮': 'z', '阝': 'z', '飞': 'z', '乙': 'z',

    # ═══ CJK 偏旁变体 ═══
    '匚': 'h', '刂': 's', '冂': 's', '亻': 'p',
    '冫': 'n', '冖': 'n', '讠': 'n', '凵': 's',
    '艹': 'h', '扌': 'h', '彡': 'p', '夂': 'p',
    '饣': 'p', '忄': 'n', '丬': 'n', '爿': 's',
    '氵': 'n', '宀': 'n', '辶': 'n', '灬': 'n',
    '礻': 'n', '肀': 'h', '钅': 'p', '疒': 'n',
    '衤': 'n',
}

# =====================================================================
#  IDS 解析工具
# =====================================================================

def get_expr_len(expr):
    if not expr:
        return 0
    if expr[0] not in IDS_OPERATORS:
        return 1
    op = expr[0]
    num_args = 2 if op in '⿰⿱⿴⿵⿶⿷⿸⿹⿺⿻' else 3
    idx = 1
    for _ in range(num_args):
        if idx >= len(expr):
            break
        if expr[idx] in IDS_OPERATORS:
            idx += get_expr_len(expr[idx:])
        else:
            idx += 1
    return idx

def iter_components(expr):
    if not expr or expr[0] not in IDS_OPERATORS:
        return
    op = expr[0]
    num_args = 2 if op in '⿰⿱⿴⿵⿶⿷⿸⿹⿺⿻' else 3
    rest = expr[1:]
    for _ in range(num_args):
        if not rest:
            break
        if rest[0] in IDS_OPERATORS:
            length = get_expr_len(rest)
            yield rest[:length]
            rest = rest[length:]
        else:
            yield rest[0]
            rest = rest[1:]

def find_radical_from_ids(ids_expr, depth=0):
    if not ids_expr or ids_expr[0] not in IDS_OPERATORS or depth > 5:
        return None
    for comp in iter_components(ids_expr):
        if not comp:
            continue
        if comp[0] in IDS_OPERATORS:
            r = find_radical_from_ids(comp, depth + 1)
            if r:
                return r
        else:
            if comp in RADICAL_CHAR_TO_CODE:
                return RADICAL_CHAR_TO_CODE[comp]
    return None

def find_first_matching_component(ids_expr, depth=0):
    """从IDS表达式中递归查找第一个匹配偏旁表的部件字符"""
    if not ids_expr or ids_expr[0] not in IDS_OPERATORS or depth > 5:
        return None
    for comp in iter_components(ids_expr):
        if not comp:
            continue
        if comp[0] in IDS_OPERATORS:
            r = find_first_matching_component(comp, depth + 1)
            if r:
                return r
        else:
            if comp in RADICAL_CHAR_TO_CODE:
                return comp
    return None

def get_first_base_comp(expr):
    """递归获取 IDS 表达式结构操作符后的第一个基本部件字"""
    if not expr:
        return None
    if expr[0] in IDS_OPERATORS:
        for comp in iter_components(expr):
            if comp:
                if comp[0] in IDS_OPERATORS:
                    return get_first_base_comp(comp)
                else:
                    return comp
    return expr[0]

def get_first_component(expr):
    """递归获取 IDS 表达式的第一个真实字符（不依赖偏旁表）"""
    if not expr:
        return None
    if expr[0] in IDS_OPERATORS:
        return get_first_component(expr[1:])
    return expr[0]

# =====================================================================
#  数据加载
# =====================================================================

def load_ids(path):
    ids_map = {}
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            if line.startswith('#'):
                continue
            parts = line.strip().split('\t')
            if len(parts) < 3:
                continue
            char = parts[1]
            for i in range(2, len(parts)):
                expr = parts[i].split('[')[0].strip()
                if expr and expr != char:
                    if char not in ids_map:
                        ids_map[char] = []
                    if expr not in ids_map[char]:
                        ids_map[char].append(expr)
    return ids_map

def load_unihan_rs(path):
    rs_map, strokes_map = {}, {}
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            if line.startswith('#'):
                continue
            parts = line.strip().split('\t')
            if len(parts) < 3:
                continue
            code_str, field, value = parts[0], parts[1], parts[2]
            try:
                ch = chr(int(code_str[2:], 16))
            except (ValueError, IndexError):
                continue
            if field == 'kRSUnicode':
                try:
                    rs_map[ch] = int(value.replace("'", "").split('.')[0])
                except (ValueError, IndexError):
                    pass
            elif field == 'kTotalStrokes':
                try:
                    strokes_map[ch] = int(value)
                except ValueError:
                    pass
    return rs_map, strokes_map

_gb2312_cache = None
def get_gb2312_chars():
    global _gb2312_cache
    if _gb2312_cache is not None:
        return _gb2312_cache
    chars = []
    for b1 in range(0xB0, 0xF8):
        for b2 in range(0xA1, 0xFF):
            try:
                ch = bytes([b1, b2]).decode('gb2312')
                if is_cjk(ch):
                    chars.append(ch)
            except (UnicodeDecodeError, ValueError):
                pass
    _gb2312_cache = chars
    return chars


# =====================================================================
#  编码核心
# =====================================================================

_pinyin_cache = {}

def get_initial(char):
    if char in _pinyin_cache:
        return _pinyin_cache[char]
    if char in MULTI_PINYIN_OVERRIDE:
        res = MULTI_PINYIN_OVERRIDE[char]
        _pinyin_cache[char] = res
        return res
    pys = pypinyin.pinyin(char, style=pypinyin.Style.NORMAL, heteronym=False)
    py = pys[0][0] if pys and pys[0] else ''
    first = py[0].lower() if py else ''
    if first == 'v':
        first = 'u'
    res = first if 'a' <= first <= 'z' else None
    _pinyin_cache[char] = res
    return res

def get_all_initials(char):
    """返回一个字的所有拼音首字母，最常用的排第一"""
    pys = pypinyin.pinyin(char, style=pypinyin.Style.NORMAL, heteronym=True)
    if not pys or not pys[0]:
        return []
    seen = []
    seen_set = set()
    for py in pys[0]:
        first = py[0].lower() if py else ''
        if first == 'v':
            first = 'u'
        if 'a' <= first <= 'z' and first not in seen_set:
            seen.append(first)
            seen_set.add(first)
    return seen

def get_char_parts(char, ids_map, rs_map, problems):
    """返回 (structure, radical)，不含拼音首字母"""
    ids_variants = ids_map.get(char, [])
    first_expr = ids_variants[0] if ids_variants else ''
    structure = None

    if char in STRUCTURE_OVERRIDES:
        structure = STRUCTURE_OVERRIDES[char]
    elif first_expr and first_expr != char:
        for expr in ids_variants:
            s = ids_to_structure(expr)
            if s:
                structure = s
                break
        if structure is None:
            problems.ids_no_struct.append((char, first_expr))
    elif ids_variants and first_expr == char:
        structure = 'y'
        problems.ids_self.append((char, first_expr))
    else:
        problems.no_ids.append(char)

    if structure is None:
        structure = 'y'
        problems.structure_default_y.append((char, first_expr or '(无IDS)'))

    if structure == 'y':
        radical = get_stroke_code(char)
        if char not in FIRST_STROKE_MAP:
            problems.stroke_default_z.append((char,))
    else:
        radical = None
        if char in CHAR_RADICAL_OVERRIDES:
            radical = CHAR_RADICAL_OVERRIDES[char]
        else:
            for expr in ids_variants:
                if expr == char:
                    continue
                r = find_radical_from_ids(expr)
                if r:
                    radical = r
                    break
        if radical is None:
            structure = 'y'
            radical = get_stroke_code(char)
            if char not in FIRST_STROKE_MAP:
                problems.stroke_default_z.append((char,))

    return structure, radical

def radical_num_to_code(radical_num):
    rchar = RADICAL_NUM_TO_CHAR.get(radical_num)
    return RADICAL_CHAR_TO_CODE.get(rchar, 'z') if rchar else 'z'

def ids_to_structure(ids_expr):
    return IDS_TO_STRUCTURE.get(ids_expr[0], None) if ids_expr else None

def get_stroke_code(char):
    return FIRST_STROKE_MAP.get(char, 'z')


def encode_char(char, ids_map, rs_map, problems):
    initial = get_initial(char)
    if initial is None:
        problems.no_pinyin.append(char)
        return None, None
    structure, radical = get_char_parts(char, ids_map, rs_map, problems)
    return initial + structure + radical, structure

def encode_word(word, char_codes):
    """词语编码: ISIR + I... (支持任意长度)"""
    n = len(word)
    if n < 2 or n > 8:
        return None
    codes = []
    for c in word:
        if c not in char_codes or len(char_codes[c]) < 3:
            return None
        codes.append(char_codes[c])

    res = codes[0][0] + codes[0][1] + codes[1][0] + codes[1][2]
    for i in range(2, n):
        res += codes[i][0]
    return res

# =====================================================================
#  比较：IDS 第一部件 vs Unihan 部首
# =====================================================================

def compare_radicals(ids_map, rs_map):
    """逐字比较 IDS 第一匹配部件 和 Unihan 康熙部首"""
    gb_chars = get_gb2312_chars()
    match_count = 0
    diff_count = 0
    no_ids_count = 0
    no_uni_count = 0
    conflicts = []

    os.makedirs(os.path.dirname(COMPARE_FILE), exist_ok=True)
    with open(COMPARE_FILE, 'w', encoding='utf-8') as f:
        f.write("=== IDS 第一匹配部件 vs Unihan 康熙部首 ===\n")
        f.write(f"规则：IDS递归查找第一个在偏旁表中的部件\n\n")
        f.write(f"{'字':>2}  {'音':>2}  {'结构':>4}"
                f"  {'IDS部件':>4}→{'码':>2}"
                f"  {'U部首':>4}→{'码':>2}"
                f"  {'结果':>6}\n")
        f.write("-" * 55 + "\n")

        for char in gb_chars:
            initial = get_initial(char)
            if initial is None:
                continue

            ids_variants = ids_map.get(char, [])
            first_expr = ids_variants[0] if ids_variants else ''

            struct = STRUCTURE_OVERRIDES.get(char)
            if struct is None and ids_variants:
                for expr in ids_variants:
                    if expr != char:
                        s = ids_to_structure(expr)
                        if s:
                            struct = s
                            break
            if struct is None:
                struct = 'y'

            ids_comp_char = None
            ids_code = None
            if ids_variants:
                for expr in ids_variants:
                    if expr == char:
                        continue
                    comp = find_first_matching_component(expr)
                    if comp:
                        ids_comp_char = comp
                        ids_code = RADICAL_CHAR_TO_CODE[comp]
                        break

            if ids_comp_char is None:
                no_ids_count += 1
                continue

            uni_char = None
            uni_code = None
            if char in rs_map:
                uni_char = RADICAL_NUM_TO_CHAR.get(rs_map[char], '?')
                uni_code = radical_num_to_code(rs_map[char])

            if uni_code is None:
                no_uni_count += 1
                continue

            if ids_code == uni_code:
                status = "SAME"
                match_count += 1
            else:
                status = "DIFF"
                diff_count += 1
                conflicts.append((
                    char, initial, struct,
                    ids_comp_char, ids_code,
                    uni_char, uni_code,
                ))

            f.write(f"{char}  {initial}  {struct:>4}"
                    f"  {ids_comp_char:>4}→{ids_code:>2}"
                    f"  {uni_char:>4}→{uni_code:>2}"
                    f"  {status:>6}\n")

        f.write(f"\n{'=' * 55}\n")
        f.write(f"汇总:\n")
        f.write(f"  一致 (SAME): {match_count}\n")
        f.write(f"  冲突 (DIFF): {diff_count}\n")
        f.write(f"  无IDS匹配部件: {no_ids_count}\n")
        f.write(f"  无Unihan部首: {no_uni_count}\n")
        if match_count + diff_count > 0:
            f.write(f"  一致率: {match_count / (match_count + diff_count) * 100:.1f}%\n")

        if conflicts:
            f.write(f"\n{'=' * 55}\n")
            f.write(f"冲突清单 ({len(conflicts)} 字)\n")
            f.write(f"{'=' * 55}\n\n")
            f.write(f"{'字':>2}  {'音':>2}  {'结构':>4}"
                    f"  {'IDS部件':>4}→{'码':>2}"
                    f"  {'U部首':>4}→{'码':>2}\n")
            f.write("-" * 45 + "\n")
            for ch, ini, st, ic, ir, uc, ur in conflicts:
                f.write(f"{ch}  {ini}  {st:>4}"
                        f"  {ic:>4}→{ir:>2}"
                        f"  {uc:>4}→{ur:>2}\n")

    print(f"比较报告: {COMPARE_FILE}")
    print(f"  一致: {match_count}, 冲突: {diff_count}, "
          f"无IDS匹配: {no_ids_count}, 无Unihan: {no_uni_count}")
    if match_count + diff_count > 0:
        print(f"  一致率: {match_count / (match_count + diff_count) * 100:.1f}%")

def check_ids_first(ids_map):
    """对比 IDS 结构码后第一位的字是否在偏旁表中，并输出信息"""
    gb_chars = get_gb2312_chars()
    match_count = 0
    no_match_count = 0
    no_comp_count = 0

    os.makedirs(os.path.dirname(IDS_FIRST_CHECK_FILE), exist_ok=True)
    with open(IDS_FIRST_CHECK_FILE, 'w', encoding='utf-8') as f:
        f.write("=== IDS 递归匹配的第一个部件 vs 偏旁表 ===\n")
        f.write(f"{'字':>2}  {'IDS表达式':<12}  {'匹配部件':>4}  {'是否匹配':>6}\n")
        f.write("-" * 40 + "\n")

        for char in gb_chars:
            ids_variants = ids_map.get(char, [])
            if not ids_variants:
                no_comp_count += 1
                continue

            first_expr = ids_variants[0]
            comp = find_first_matching_component(first_expr)

            if not comp:
                no_match_count += 1
                f.write(f"{char}  {first_expr:<12}  {'无':>4}  {'否':>6}\n")
                continue

            if comp in RADICAL_CHAR_TO_CODE:
                match_count += 1
                status = "是"
            else:
                no_match_count += 1
                status = "否"

            f.write(f"{char}  {first_expr:<12}  {comp:>4}  {status:>6}\n")

        f.write(f"\n汇总: 匹配 {match_count}, 不匹配 {no_match_count}, 无IDS {no_comp_count}\n")

    print(f"检查报告已输出至: {IDS_FIRST_CHECK_FILE}")
    print(f"  匹配偏旁表: {match_count} 字")
    print(f"  不匹配偏旁表: {no_match_count} 字")
    print(f"  无IDS数据: {no_comp_count} 字")

def list_self_ids():
    """导出 IDS 表达式中没有结构运算符（即表达式为自身）的 GB2312 字符列表"""
    if not os.path.exists(IDS_FILE):
        print(f"错误: 缺少必需文件 {IDS_FILE}")
        sys.exit(1)

    gb_chars = get_gb2312_chars()
    self_ids_chars = []

    with open(IDS_FILE, 'r', encoding='utf-8') as f:
        for line in f:
            if line.startswith('#'):
                continue
            parts = line.strip().split('\t')
            if len(parts) < 3:
                continue
            char = parts[1]
            if char not in gb_chars:
                continue

            exprs = [p.split('[')[0].strip() for p in parts[2:]]

            has_structure = False
            for expr in exprs:
                if expr and expr != char and any(op in expr for op in IDS_OPERATORS):
                    has_structure = True
                    break

            if not has_structure:
                self_ids_chars.append(char)

    os.makedirs(os.path.dirname(NO_IDS_FILE), exist_ok=True)
    with open(NO_IDS_FILE, 'w', encoding='utf-8') as f:
        f.write(f"=== IDS 无结构运算符的 GB2312 字符 (共 {len(self_ids_chars)} 字) ===\n\n")
        for char in self_ids_chars:
            f.write(f"{char}\n")

    print(f"无结构体字符列表已输出至: {NO_IDS_FILE}")
    print(f"  共计: {len(self_ids_chars)} 字")

# =====================================================================
#  IDS 文件差异比较 (原 inspect_ids.py 的 diff 功能)
# =====================================================================

def diff_ids(file_a, file_b):
    """比较两个 IDS 文件的差异，输出变更报告"""
    REPORT_DIFF_FILE = os.path.join(ROOT, "dicts/reports", "ids_diff.txt")

    def load_ids_full(path):
        """加载 IDS 文件，返回 {char: [expr, ...]}，保留所有表达式"""
        ids_map = {}
        with open(path, 'r', encoding='utf-8') as f:
            for line in f:
                if line.startswith('#'):
                    continue
                parts = line.strip().split('\t')
                if len(parts) < 3:
                    continue
                char = parts[1]
                exprs = []
                for i in range(2, len(parts)):
                    expr = parts[i].split('[')[0].strip()
                    if expr and expr != char:
                        if expr not in exprs:
                            exprs.append(expr)
                if exprs:
                    ids_map[char] = exprs
        return ids_map

    name_a = os.path.basename(os.path.abspath(file_a))
    name_b = os.path.basename(os.path.abspath(file_b))

    if not os.path.exists(file_a):
        print(f"错误: 找不到 {file_a}")
        sys.exit(1)
    if not os.path.exists(file_b):
        print(f"错误: 找不到 {file_b}")
        sys.exit(1)

    map_a = load_ids_full(file_a)
    map_b = load_ids_full(file_b)

    chars_a = set(map_a.keys())
    chars_b = set(map_b.keys())

    only_a = sorted(chars_a - chars_b)
    only_b = sorted(chars_b - chars_a)
    common = sorted(chars_a & chars_b)

    changed = []
    struct_flip = []

    for char in common:
        exprs_a = map_a[char]
        exprs_b = map_b[char]
        if exprs_a == exprs_b:
            continue
        changed.append((char, exprs_a, exprs_b))

        for ea in exprs_a:
            sa = ids_to_structure(ea)
            if sa:
                break
        else:
            sa = 'y'
        for eb in exprs_b:
            sb = ids_to_structure(eb)
            if sb:
                break
        else:
            sb = 'y'
        if sa != sb:
            struct_flip.append((char, exprs_a, sa, exprs_b, sb))

    os.makedirs(os.path.dirname(REPORT_DIFF_FILE), exist_ok=True)
    with open(REPORT_DIFF_FILE, 'w', encoding='utf-8') as f:
        f.write("=== IDS 差异报告 ===\n")
        f.write(f"文件A: {file_a} ({len(map_a)} 条)\n")
        f.write(f"文件B: {file_b} ({len(map_b)} 条)\n\n")

        f.write("=" * 60 + "\n")
        f.write("汇总\n")
        f.write("=" * 60 + "\n")
        f.write(f"  仅A有: {len(only_a)} 字\n")
        f.write(f"  仅B有: {len(only_b)} 字\n")
        f.write(f"  表达式不同: {len(changed)} 字\n")
        f.write(f"  结构码变化: {len(struct_flip)} 字\n")
        f.write(f"  完全一致: {len(common) - len(changed)} 字\n\n")

        if struct_flip:
            f.write("=" * 60 + "\n")
            f.write(f"[结构码变化] {len(struct_flip)} 字  ← 影响编码\n")
            f.write("=" * 60 + "\n")
            f.write(f"{'字':>2}  {'A结构':>4}  {'A表达式':<16}"
                    f"  {'B结构':>4}  {'B表达式':<16}\n")
            f.write("-" * 60 + "\n")
            for char, ea, sa, eb, sb in struct_flip:
                f.write(f"{char}  {sa:>4}  {ea[0]:<16}"
                        f"  {sb:>4}  {eb[0]:<16}\n")

        if only_a:
            f.write(f"\n{'=' * 60}\n")
            f.write(f"[仅A有] {name_a} 独有 ({len(only_a)} 字)\n")
            f.write(f"{'=' * 60}\n")
            for char in only_a:
                f.write(f"  {char}  {map_a[char][0]}\n")

        if only_b:
            f.write(f"\n{'=' * 60}\n")
            f.write(f"[仅B有] {name_b} 独有 ({len(only_b)} 字)\n")
            f.write(f"{'=' * 60}\n")
            for char in only_b:
                f.write(f"  {char}  {map_b[char][0]}\n")

        diff_only = [c for c in changed
                     if not any(c[0] == sf[0] for sf in struct_flip)]
        if diff_only:
            f.write(f"\n{'=' * 60}\n")
            f.write(f"[表达式不同] 结构未变 ({len(diff_only)} 字)\n")
            f.write(f"{'=' * 60}\n")
            f.write(f"{'字':>2}  {'A表达式':<16}  {'B表达式':<16}\n")
            f.write("-" * 45 + "\n")
            for char, ea, eb in diff_only:
                f.write(f"{char}  {ea[0]:<16}  {eb[0]:<16}\n")

    print(f"IDS 差异报告: {REPORT_DIFF_FILE}")
    print(f"  文件A: {name_a} ({len(map_a)} 条)")
    print(f"  文件B: {name_b} ({len(map_b)} 条)")
    print(f"  仅A有: {len(only_a)}, 仅B有: {len(only_b)}")
    print(f"  表达式不同: {len(changed)}, 结构码变化: {len(struct_flip)}")
    print(f"  完全一致: {len(common) - len(changed)}")


# =====================================================================
#  IDS 深度结构分析 (原 inspect_ids.py)
# =====================================================================

def inspect_ids():
    """IDS 数据库深度结构分析"""
    if not os.path.exists(IDS_FILE):
        print(f"错误: 找不到文件 {IDS_FILE}")
        sys.exit(1)

    total_lines = 0
    cjk_chars = 0

    no_ids = 0
    ids_self = []
    ids_normal = []
    ids_nested = []
    ids_weird_start = []

    comp_standard = 0
    comp_ext = []
    comp_symbol = []

    overlap_total = 0
    overlap_extractable = 0

    op_counter = Counter()

    with open(IDS_FILE, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('#'):
                continue

            total_lines += 1
            parts = line.split('\t')
            if len(parts) < 3:
                continue

            char = parts[1]
            if not is_cjk(char):
                continue

            cjk_chars += 1
            ids_expr = parts[2].split(' ')[0].strip()
            if '[' in ids_expr:
                ids_expr = ids_expr.split('[')[0].strip()

            if not ids_expr:
                no_ids += 1
                continue

            if ids_expr == char:
                ids_self.append(char)
                continue

            if ids_expr[0] in IDS_OPERATORS:
                op_counter[ids_expr[0]] += 1

                if len(ids_expr) > 1 and ids_expr[1] in IDS_OPERATORS:
                    ids_nested.append((char, ids_expr))
                else:
                    ids_normal.append((char, ids_expr))

                comp = get_first_component(ids_expr)
                if comp:
                    if is_cjk(comp):
                        comp_standard += 1
                        if ids_expr[0] == '⿻':
                            overlap_total += 1
                            overlap_extractable += 1
                    elif is_ext_cjk(comp):
                        comp_ext.append((char, ids_expr, comp))
                        if ids_expr[0] == '⿻':
                            overlap_total += 1
                    else:
                        comp_symbol.append((char, ids_expr, comp))
                        if ids_expr[0] == '⿻':
                            overlap_total += 1
            else:
                ids_weird_start.append((char, ids_expr))

    print("=" * 60)
    print("  IDS 数据库深度结构分析报告")
    print("=" * 60)
    print(f"扫描总行数: {total_lines}")
    print(f"其中 CJK 基本区汉字: {cjk_chars}")
    print("-" * 60)

    print(f"\n[1] 完全无 IDS 数据: {no_ids} 字")
    print(f"[2] IDS 映射到自身 (真独体字): {len(ids_self)} 字")
    print(f"[3] 正常合体字: {len(ids_normal)} 字")
    print(f"[4] 嵌套结构: {len(ids_nested)} 字")
    print(f"[5] 异常开头 (无操作符且非自身): {len(ids_weird_start)} 字")

    print("\n" + "=" * 60)
    print("  首部件提取可行性分析 (核心)")
    print("=" * 60)
    print(f"成功提取到常见 CJK 部件: {comp_standard} 字")
    print(f"首部件是生僻扩展字 (可能需补全映射): {len(comp_ext)} 字")
    if comp_ext:
        print(f"    示例: {' '.join([f'{c}({e}->{comp})' for c,e,comp in comp_ext[:10]])}")

    print(f"\n首部件是特殊符号 (无法映射，需兜底): {len(comp_symbol)} 字")
    if comp_symbol:
        print(f"    示例: {' '.join([f'{c}({e}->{comp})' for c,e,comp in comp_symbol[:10]])}")

    print("\n" + "=" * 60)
    print("  ⿻ (交叠结构) 专项检查")
    print("=" * 60)
    print(f"总 ⿻ 结构字数: {overlap_total}")
    print(f"其中能成功提取出 CJK 首部件: {overlap_extractable}")
    print("-> 结论：如果实现 '⿻ 允许提取首部件' 的逻辑，这 {} 个字将不再退化为首笔画。".format(overlap_extractable))

    print("\n" + "=" * 60)
    print("  操作符使用频率")
    print("=" * 60)
    for op, count in op_counter.most_common():
        print(f"    {op} : {count}")

# =====================================================================
#  问题收集器
# =====================================================================

class ProblemCollector:
    def __init__(self):
        self.no_pinyin = []
        self.no_ids = []
        self.ids_self = []
        self.ids_no_struct = []
        self.stroke_default_z = []
        self.structure_default_y = []

    def summary(self):
        return sum(len(v) for v in self.__dict__.values())


# =====================================================================
#  生成函数
# =====================================================================

def generate_chars(ids_map, rs_map, strokes_map):
    problems = ProblemCollector()
    gb_chars = get_gb2312_chars()
    print(f"加载数据... IDS: {len(ids_map)}, Unihan部首: {len(rs_map)}, "
          f"GB2312: {len(gb_chars)} 字")

    entries = []
    stats = {
        'total': 0, 'structure': Counter(),
        'radical': Counter(), 'buckets': {},
    }

    jieba_char_freq = {}
    if os.path.exists(JIEBA_FILE):
        with open(JIEBA_FILE, 'r', encoding='utf-8') as f:
            for line in f:
                parts = line.strip().split(' ')
                if len(parts) >= 2 and len(parts[0]) == 1 and is_cjk(parts[0]):
                    try:
                        freq = int(parts[1])
                        jieba_char_freq[parts[0]] = max(
                            jieba_char_freq.get(parts[0], 0), freq)
                    except ValueError:
                        pass

    subtlex_char_freq = {}
    if os.path.exists(SUBTLEX_FILE):
        try:
            import openpyxl
            wb = openpyxl.load_workbook(SUBTLEX_FILE, read_only=True)
            ws = wb.active
            for i, row in enumerate(ws.iter_rows(values_only=True)):
                if i < 3: continue
                w, wcount = row[0], row[1]
                if w and isinstance(w, str) and len(w) == 1 and is_cjk(w):
                    subtlex_char_freq[w] = wcount or 0
            print(f"  SUBTLEX字频: {len(subtlex_char_freq)} 字")
        except Exception as e:
            print(f"  SUBTLEX字频加载失败: {e}")

    HETERO_THRESHOLD = 100000

    for char in gb_chars:
        try:
            structure, radical = get_char_parts(char, ids_map, rs_map, problems)
        except Exception as e:
            print(f"警告：字符 {char} 解析失败: {e}")
            continue

        all_initials = get_all_initials(char)
        if not all_initials:
            problems.no_pinyin.append(char)
            continue

        freq = subtlex_char_freq.get(char, 0) or jieba_char_freq.get(char, 0)
        if freq >= HETERO_THRESHOLD:
            seen_initials = set()
            initials = []
            for ini in all_initials:
                if ini not in seen_initials:
                    initials.append(ini)
                    seen_initials.add(ini)
        else:
            initials = all_initials[:1]

        for initial in initials:
            code = initial + structure + radical
            stats['total'] += 1
            stats['structure'][structure] += 1
            stats['radical'][code[2]] += 1
            stats['buckets'].setdefault(code, []).append(char)
            entries.append((char, code, strokes_map.get(char, 10)))

    max_freq = max(jieba_char_freq.values()) if jieba_char_freq else 1
    final_entries = []
    for char, code, strokes in entries:
        if char in jieba_char_freq:
            norm = math.log(jieba_char_freq[char] + 1) / math.log(max_freq + 1)
            weight = int(10 + norm * 990)
        else:
            weight = max(10, 200 - strokes * 5)
        final_entries.append((char, code, weight))

    os.makedirs(os.path.dirname(CHARS_FILE), exist_ok=True)
    final_entries.sort(key=lambda x: (-x[2], x[1]))
    with open(CHARS_FILE, 'w', encoding='utf-8') as f:
        for char, code, weight in final_entries:
            f.write(f"{char}\t{code}\t{weight}\n")

    print(f"\n输出单字码表: {CHARS_FILE} ({stats['total']} 字)")
    print(f"问题字总计: {problems.summary()} 条")
    return final_entries, stats, problems

def generate_words(char_entries, min_freq=0):
    print(f"\n生成词语码表 (词频阈值: {min_freq})...")
    char_codes = {}
    for char, code, _ in char_entries:
        if char not in char_codes:
            char_codes[char] = code
    words = {}

    if os.path.exists(JIEBA_FILE):
        with open(JIEBA_FILE, 'r', encoding='utf-8') as f:
            for line in f:
                parts = line.strip().split(' ')
                if len(parts) < 2:
                    continue
                word = parts[0]
                if (len(word) < 2 or len(word) > 4
                        or not all(is_cjk(c) for c in word)):
                    continue
                try:
                    freq = int(parts[1])
                except ValueError:
                    continue
                if freq < min_freq:
                    continue
                code = encode_word(word, char_codes)
                if not code:
                    continue
                if word not in words or freq > words[word][1]:
                    words[word] = (code, freq)

    base_multiplier = {2: 2.6, 3: 3.6, 4: 4.5}
    with open(WORDS_FILE, 'w', encoding='utf-8') as f:
        for word, (code, freq) in sorted(words.items(), key=lambda x: -x[1][1]):
            scaled = min(100000, max(100,
                         int(math.log(freq + 1) * 5000
                             * base_multiplier[len(word)])))
            f.write(f"{word}\t{code}\t{scaled}\n")

    print(f"输出词语码表: {WORDS_FILE} ({len(words)} 词)")

    buckets_2 = {}
    for word, (code, _) in words.items():
        if len(word) == 2:
            buckets_2.setdefault(code, []).append(word)
    if buckets_2:
        total_2 = sum(len(v) for v in buckets_2.values())
        colliding = sum(1 for v in buckets_2.values() if len(v) > 1)
        max_b = max(len(v) for v in buckets_2.values())
        print(f"  2字词: {total_2} 词, {len(buckets_2)} 码位, "
              f"重码 {colliding}/{len(buckets_2)} "
              f"({colliding / len(buckets_2) * 100:.1f}%), "
              f"最大桶 {max_b}")

    buckets_3 = {}
    for word, (code, _) in words.items():
        if len(word) == 3:
            buckets_3.setdefault(code, []).append(word)
    if buckets_3:
        colliding_3 = sum(1 for v in buckets_3.values() if len(v) > 1)
        print(f"  3字词: {sum(len(v) for v in buckets_3.values())} 词, "
              f"重码桶 {colliding_3}/{len(buckets_3)}")

    return len(words)

def generate_words_subtlex(char_entries):
    print(f"\n从 SUBTLEX-CH 生成词语码表...")
    try:
        import openpyxl
    except ImportError:
        print("错误: 需要 openpyxl, pip install openpyxl")
        return 0

    if not os.path.exists(SUBTLEX_FILE):
        print(f"错误: 找不到 {SUBTLEX_FILE}")
        return 0

    char_codes = {}
    for char, code, _ in char_entries:
        if char not in char_codes:
            char_codes[char] = code

    wb = openpyxl.load_workbook(SUBTLEX_FILE, read_only=True)
    ws = wb.active
    words = []
    for i, row in enumerate(ws.iter_rows(values_only=True)):
        if i < 3:
            continue
        word, wcount = row[0], row[1]
        if not word or not isinstance(word, str):
            continue
        if len(word) < 2 or len(word) > 4:
            continue
        if not all(is_cjk(c) for c in word):
            continue
        if not all(c in char_codes for c in word):
            continue
        words.append((word, wcount or 0))

    words.sort(key=lambda x: -x[1])
    print(f"  有效词语: {len(words)}")

    base_multiplier = {2: 2.6, 3: 3.6, 4: 4.5}
    buckets = {}
    os.makedirs(os.path.dirname(WORDS_FILE), exist_ok=True)
    with open(WORDS_FILE, 'w', encoding='utf-8') as f:
        for word, freq in words:
            code = encode_word(word, char_codes)
            if not code:
                continue
            multiplier = base_multiplier.get(len(word), 2.6)
            weight = min(100000, max(100,
                         int(math.log(freq + 1) * 5000 * multiplier)))
            f.write(f"{word}\t{code}\t{weight}\n")
            buckets.setdefault(code, []).append(word)

    total = sum(len(v) for v in buckets.values())
    colliding = sum(1 for v in buckets.values() if len(v) > 1)
    print(f"  输出: {WORDS_FILE} ({total} 词)")
    print(f"  码位: {len(buckets)}, 重码: {colliding} "
          f"({colliding / len(buckets) * 100:.1f}%)")
    return total

def generate_report(stats, problems):
    sorted_buckets = sorted(stats['buckets'].items(), key=lambda x: -len(x[1]))
    no_col = sum(1 for v in stats['buckets'].values() if len(v) == 1)

    with open(REPORT_FILE, 'w', encoding='utf-8') as f:
        f.write("=== 音形码单字码表报告 ===\n\n")
        f.write(f"总字数: {stats['total']}\n")
        f.write("结构: s=左右 u=上下 w=包围 y=独体\n\n")

        f.write("=== 结构分布 ===\n")
        for s, count in stats['structure'].most_common():
            f.write(f"  {s}: {count} ({count / stats['total'] * 100:.1f}%)\n")

        f.write(f"\n=== 偏旁/笔画分布 ===\n")
        for r, count in stats['radical'].most_common():
            f.write(f"  {r}: {count} ({count / stats['total'] * 100:.1f}%)\n")

        f.write(f"\n=== 编码桶分析 ===\n")
        f.write(f"桶总数: {len(stats['buckets'])}\n")
        if sorted_buckets:
            f.write(f"最大桶: {sorted_buckets[0][0]} = "
                    f"{len(sorted_buckets[0][1])} 字\n")
            f.write(f"  内容: {''.join(sorted_buckets[0][1][:20])}\n")
        avg = stats['total'] / len(stats['buckets']) if stats['buckets'] else 0
        f.write(f"平均桶大小: {avg:.1f}\n")
        pct = no_col / len(stats['buckets']) * 100 if stats['buckets'] else 0
        f.write(f"无重码桶: {no_col}/{len(stats['buckets'])} ({pct:.1f}%)\n\n")

        f.write("=== Top 30 最大重码桶 ===\n")
        for code, cb in sorted_buckets[:30]:
            f.write(f"  {code}: {len(cb)} 字  {''.join(cb[:15])}\n")

        f.write("\n" + "=" * 60
                + "\n  问题字清单（需人工校对）\n" + "=" * 60 + "\n\n")
        f.write(f"[1] 无拼音: {len(problems.no_pinyin)} 字\n")
        f.write(f"[2] 无IDS数据: {len(problems.no_ids)} 字\n")
        f.write(f"[3] IDS映射到自身(默认独体y): "
                f"{len(problems.ids_self)} 字\n")
        f.write(f"[4] IDS无结构操作符(默认独体y): "
                f"{len(problems.ids_no_struct)} 字\n")
        f.write(f"\n[6] 独体字首笔画无映射(默认z): "
                f"{len(problems.stroke_default_z)} 字\n")
        for entry in problems.stroke_default_z:
            ch = entry[0] if isinstance(entry, tuple) else entry
            f.write(f"    {ch}\n")
        f.write(f"\n问题字总计: {problems.summary()} 条\n")

    print(f"生成统计报告: {REPORT_FILE}")

def report_radicals():
    """统计每个偏旁码的使用频率和对应的偏旁字符"""
    code_to_chars = {}
    for ch, code in RADICAL_CHAR_TO_CODE.items():
        code_to_chars.setdefault(code, []).append(ch)

    radical_freq = {}
    bucket_detail = {}
    if os.path.exists(CHARS_FILE):
        with open(CHARS_FILE, 'r', encoding='utf-8') as f:
            for line in f:
                parts = line.strip().split('\t')
                if len(parts) < 2 or len(parts[0]) != 1:
                    continue
                code = parts[1]
                if len(code) < 3:
                    continue
                radical = code[2]
                radical_freq[radical] = radical_freq.get(radical, 0) + 1
                struct = code[1] if len(code) > 1 else '?'
                key = f"{code[0]}{struct}{radical}"
                bucket_detail.setdefault(key, 0)
                bucket_detail[key] += 1
    else:
        print("错误: 需要先生成单字码表 (--chars)")
        return

    total = sum(radical_freq.values())
    target = total / len(radical_freq) if radical_freq else 0
    sorted_freq = sorted(radical_freq.items(), key=lambda x: -x[1])

    REPORT_RADICALS_FILE = os.path.join(ROOT, "dicts/radical_report.txt")
    os.makedirs(os.path.dirname(REPORT_RADICALS_FILE), exist_ok=True)

    with open(REPORT_RADICALS_FILE, 'w', encoding='utf-8') as f:
        f.write("=== 偏旁码频率报告 ===\n\n")
        f.write(f"总字数: {total}\n")
        f.write(f"偏旁码数: {len(radical_freq)}\n")
        f.write(f"理想平均: {target:.1f} 字/码\n")
        f.write(f"最大/最小: {max(radical_freq.values())} / {min(radical_freq.values())}\n\n")

        f.write(f"{'码':>4}  {'字数':>5}  {'偏差':>7}  {'占比':>6}  偏旁字符\n")
        f.write("-" * 70 + "\n")

        for code, count in sorted_freq:
            pct = count / total * 100
            deviation = count - target
            chars = code_to_chars.get(code, [])
            sample = " ".join(chars[:8])
            if len(chars) > 8:
                sample += f" ...(+{len(chars) - 8})"
            f.write(f"{code:>4}  {count:>5}  {deviation:>+7.1f}  {pct:>5.1f}%  {sample}\n")

        f.write(f"\n\n=== Top 30 最拥挤的 音+结构+偏旁 组合 ===\n\n")
        sorted_buckets = sorted(bucket_detail.items(), key=lambda x: -x[1])
        for key, count in sorted_buckets[:30]:
            f.write(f"  {key}: {count} 字\n")

    print(f"\n偏旁码频率报告: {REPORT_RADICALS_FILE}")
    print(f"总字数: {total}, 偏旁码数: {len(radical_freq)}, 理想平均: {target:.1f}")
    print(f"\n{'码':>4}  {'字数':>5}  {'偏差':>7}  {'占比':>6}  偏旁字符")
    print("-" * 70)
    for code, count in sorted_freq:
        pct = count / total * 100
        deviation = count - target
        chars = code_to_chars.get(code, [])
        sample = " ".join(chars[:6])
        if len(chars) > 6:
            sample += f" ...(+{len(chars) - 6})"
        marker = " ← 太密" if deviation > target * 0.5 else (" ← 太稀" if deviation < -target * 0.5 else "")
        print(f"{code:>4}  {count:>5}  {deviation:>+7.1f}  {pct:>5.1f}%  {sample}{marker}")

def verify_char(char, ids_map, rs_map):
    problems = ProblemCollector()
    code, struct = encode_char(char, ids_map, rs_map, problems)
    print(f"字符: {char}")
    print(f"编码: {code}")
    print(f"结构: {struct}  (s左右 u上下 w包围 y独体)")
    print(f"音首: {get_initial(char)}")

    if struct == 'y':
        print(f"首笔: {get_stroke_code(char)}  (h横 n捺 p撇 s竖 z折)")
    else:
        ids_variants = ids_map.get(char, [])
        if ids_variants:
            print(f"IDS:  {ids_variants[0]}")
            comp = find_first_matching_component(ids_variants[0])
            if comp:
                print(f"匹配部件: {comp} → {RADICAL_CHAR_TO_CODE[comp]}")
        if char in rs_map:
            rchar = RADICAL_NUM_TO_CHAR.get(rs_map[char], '?')
            rcode = radical_num_to_code(rs_map[char])
            print(f"Unihan: {rchar} → {rcode}")

    if problems.summary() > 0:
        print(f"\n问题: {problems.summary()} 条")
        for attr in vars(problems):
            lst = getattr(problems, attr)
            if lst:
                sample = [x[0] if isinstance(x, tuple) else x for x in lst[:5]]
                print(f"  {attr}: {sample}")

    print("-" * 40)

def benchmark_words(char_entries):
    """测试不同词频阈值下的重码率"""
    char_codes = {}
    for char, code, _ in char_entries:
        if char not in char_codes:
            char_codes[char] = code

    all_words = {}
    if os.path.exists(JIEBA_FILE):
        with open(JIEBA_FILE, 'r', encoding='utf-8') as f:
            for line in f:
                parts = line.strip().split(' ')
                if len(parts) < 2:
                    continue
                word = parts[0]
                if (len(word) < 2 or len(word) > 4
                        or not all(is_cjk(c) for c in word)):
                    continue
                try:
                    freq = int(parts[1])
                except ValueError:
                    continue
                code = encode_word(word, char_codes)
                if not code:
                    continue
                if word not in all_words or freq > all_words[word][1]:
                    all_words[word] = (code, freq)

    print(f"总加载词数: {len(all_words)}")
    print()

    thresholds = [0, 10, 50, 100, 200, 500, 1000, 2000, 5000, 10000]

    print(f"{'阈值':>6}  {'词数':>8}  {'2字词':>8}  {'码位':>7}  "
          f"{'重码率':>8}  {'最大桶':>6}")
    print("-" * 60)

    for thresh in thresholds:
        filtered = {w: (c, f) for w, (c, f) in all_words.items() if f >= thresh}

        buckets_2 = {}
        for word, (code, _) in filtered.items():
            if len(word) == 2:
                buckets_2.setdefault(code, []).append(word)

        if not buckets_2:
            print(f"{thresh:>6}  {len(filtered):>8}  {'0':>8}  {'0':>7}  "
                  f"{'N/A':>8}  {'N/A':>6}")
            continue

        total_2 = sum(len(v) for v in buckets_2.values())
        colliding = sum(1 for v in buckets_2.values() if len(v) > 1)
        max_b = max(len(v) for v in buckets_2.values())
        rate = colliding / len(buckets_2) * 100

        print(f"{thresh:>6}  {len(filtered):>8}  {total_2:>8}  "
              f"{len(buckets_2):>7}  {rate:>7.1f}%  {max_b:>6}")

    print()

# =====================================================================
#  入口
# =====================================================================

def main():
    parser = argparse.ArgumentParser(description='音形码词库生成工具')
    parser.add_argument('--chars', action='store_true', help='生成单字码表')
    parser.add_argument('--words', action='store_true', help='生成词语码表')
    parser.add_argument('--report', action='store_true', help='生成统计报告')
    parser.add_argument('--all', action='store_true', help='生成全部（默认）')
    parser.add_argument('--verify', type=str, metavar='字', help='校验单个汉字编码')
    parser.add_argument('--compare', action='store_true', help='比较IDS vs Unihan')
    parser.add_argument('--check-first', action='store_true', help='检查IDS结构码后第一位是否匹配偏旁')
    parser.add_argument('--list-no-ids', action='store_true', help='导出无结构运算符的字符列表')
    parser.add_argument('--inspect', action='store_true', help='IDS数据库深度结构分析')
    parser.add_argument('--diff-ids', nargs=2, metavar='FILE', help='比较两个IDS文件的差异')
    parser.add_argument('--report-radicals', action='store_true', help='偏旁码频率分析')
    parser.add_argument('--benchmark', action='store_true', help='词频阈值基准测试')
    parser.add_argument('--words-source', type=str, default='subtlex', choices=['subtlex', 'jieba', 'none'], help='词语数据源: subtlex(SUBTLEX-CH), jieba(jieba词库), none(纯单字)')
    parser.add_argument('--min-freq', type=int, default=200, help='词语最低词频')
    args = parser.parse_args()

    if not any([args.chars, args.words, args.report, args.all,
                args.verify, args.compare, args.benchmark]):
        args.all = True

    # ---- 独立分析/诊断命令（不依赖完整生成流程） ----

    if args.inspect:
        inspect_ids()
        return

    if args.diff_ids:
        diff_ids(args.diff_ids[0], args.diff_ids[1])
        return

    if args.report_radicals:
        report_radicals()
        return

    if args.benchmark:
        if not os.path.exists(CHARS_FILE):
            print("错误: 需要先生成单字码表 (--chars)")
            sys.exit(1)
        entries = []
        with open(CHARS_FILE, 'r', encoding='utf-8') as f:
            for line in f:
                p = line.strip().split('\t')
                if len(p) >= 3:
                    entries.append((p[0], p[1], int(p[2])))
        benchmark_words(entries)
        return

    if args.verify:
        for f in [IDS_FILE, UNIHAN_FILE]:
            if not os.path.exists(f):
                print(f"错误: 缺少必需文件 {f}")
                sys.exit(1)
        ids_map = load_ids(IDS_FILE)
        rs_map, _ = load_unihan_rs(UNIHAN_FILE)
        verify_char(args.verify, ids_map, rs_map)
        return

    if args.compare:
        for f in [IDS_FILE, UNIHAN_FILE]:
            if not os.path.exists(f):
                print(f"错误: 缺少必需文件 {f}")
                sys.exit(1)
        ids_map = load_ids(IDS_FILE)
        rs_map, _ = load_unihan_rs(UNIHAN_FILE)
        compare_radicals(ids_map, rs_map)
        return

    if args.check_first:
        if not os.path.exists(IDS_FILE):
            print(f"错误: 缺少必需文件 {IDS_FILE}")
            sys.exit(1)
        ids_map = load_ids(IDS_FILE)
        check_ids_first(ids_map)
        return

    if args.list_no_ids:
        list_self_ids()
        return

    # ---- 完整生成流程 ----

    for f in [IDS_FILE, UNIHAN_FILE, JIEBA_FILE]:
        if not os.path.exists(f):
            print(f"错误: 找不到必需文件 {f}")
            sys.exit(1)

    print("=" * 55)
    print("  音形码词库生成工具")
    print("=" * 55)

    ids_map = load_ids(IDS_FILE)
    rs_map, strokes_map = load_unihan_rs(UNIHAN_FILE)

    entries, stats, problems = None, None, None

    if args.chars or args.all:
        entries, stats, problems = generate_chars(ids_map, rs_map, strokes_map)

    if args.words or args.all:
        if entries is None:
            entries = []
            if os.path.exists(CHARS_FILE):
                with open(CHARS_FILE, 'r', encoding='utf-8') as f:
                    for line in f:
                        p = line.strip().split('\t')
                        if len(p) >= 3:
                            entries.append((p[0], p[1], int(p[2])))
        if args.words_source == 'subtlex':
            generate_words_subtlex(entries)
        elif args.words_source == 'jieba':
            generate_words(entries, min_freq=args.min_freq)
        else:
            print("\n跳过词语码表 (--words-source none)")


    if args.report or args.all:
        if stats is None or problems is None:
            print("\n[跳过报告] 需先生成单字码表 (--chars)")
        else:
            generate_report(stats, problems)

    if args.chars or args.words or args.all:
        print("\n部署字典...")
        home = os.path.expanduser('~')
        target_dir = os.path.join(home, '.config', 'yinxing', 'dicts')
        os.makedirs(target_dir, exist_ok=True)

        deployed = []
        if os.path.exists(CHARS_FILE):
            shutil.copy(CHARS_FILE, target_dir)
            deployed.append('chars.txt')
        if os.path.exists(WORDS_FILE):
            shutil.copy(WORDS_FILE, target_dir)
            deployed.append('words.txt')

        if deployed:
            print(f"  已自动复制 {', '.join(deployed)} 到 {target_dir}")
        else:
            print("  未找到生成的字典文件，跳过部署")

    print("\n全部完成!")


if __name__ == '__main__':
    main()
