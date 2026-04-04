#!/usr/bin/env python3
import re

path = '/home/rust/Star_Shuttle/src/components/SettingsModal.svelte'
content = open(path, 'r', encoding='utf-8').read()

# 1. Add AI tab to tabs array
old1 = "    { id: 'security', label: '安全' }\n  ];"
new1 = "    { id: 'security', label: '安全' },\n    { id: 'ai', label: 'AI 助手' }\n  ];"
content = content.replace(old1, new1)
print('tabs patched:', "id: 'ai'" in content)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('done')
