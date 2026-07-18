#!/usr/bin/env python3
"""IDS 数据库深度结构分析工具"""

import os
import sys
from collections import Counter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
IDS_FILE = os.path.join(ROOT, "ids.txt")

IDS_OPERATORS = set('⿰⿱⿲⿳⿴⿵⿶⿷⿸⿹⿺⿻')

def is_cjk(char):
    return '\u4e00' <= char <= '\u9fff'

def is_ext_cjk(char):
    # CJK 扩展区 (生僻字)
    return '\u3400' <= char <= '\u4dbf' or '\U00020000' <= char <= '\U0002a6df'

def get_first_component(expr):
    """递归获取 IDS 表达式的第一个真实字符"""
    if not expr:
        return None
    if expr[0] in IDS_OPERATORS:
        return get_first_component(expr[1:])
    return expr[0]

def main():
    if not os.path.exists(IDS_FILE):
        print(f"错误: 找不到文件 {IDS_FILE}")
        sys.exit(1)

    total_lines = 0
    cjk_chars = 0

    # 基础统计
    no_ids = 0
    ids_self = []
    ids_normal = []
    ids_nested = []
    ids_weird_start = []  # 不以操作符开头，也不等于自身

    # 深度统计 (针对首部件提取)
    comp_standard = 0      # 首部件是常见 CJK
    comp_ext = []          # 首部件是生僻扩展字 (可能导致映射失败)
    comp_symbol = []       # 首部件是特殊符号 (如 ②, *)

    # ⿻ 交叠结构专项统计
    overlap_total = 0
    overlap_extractable = 0  # 能够提取出 CJK 首部件的

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

                # 深度：提取首部件
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

if __name__ == '__main__':
    main()
