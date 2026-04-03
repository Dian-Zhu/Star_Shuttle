#!/usr/bin/env python3
path = '/home/rust/Star_Shuttle/src/components/Layout.svelte'
content = open(path, 'r', encoding='utf-8').read()

# 1. Add rightSidebarEl binding ref after RightSidebar import
old1 = "  import RightSidebar from './RightSidebar.svelte';"
new1 = "  import RightSidebar from './RightSidebar.svelte';\n  let rightSidebarEl: RightSidebar;"
content = content.replace(old1, new1)

# 2. Add Ctrl+Shift+A shortcut after toggleFileBrowser block
old2 = """    // Toggle File Browser
    if (matchShortcut(event, shortcuts.toggleFileBrowser)) {
      event.preventDefault();
      isRightSidebarOpen.update(v => !v);
      return;
    }"""
new2 = """    // Toggle File Browser
    if (matchShortcut(event, shortcuts.toggleFileBrowser)) {
      event.preventDefault();
      isRightSidebarOpen.update(v => !v);
      return;
    }

    // Toggle AI Panel (Ctrl+Shift+A)
    if (event.ctrlKey && event.shiftKey && event.key === 'A') {
      event.preventDefault();
      isRightSidebarOpen.set(true);
      rightSidebarEl?.switchToAi();
      return;
    }"""
content = content.replace(old2, new2)

# 3. Bind rightSidebarEl to RightSidebar component
old3 = "            <RightSidebar />"
new3 = "            <RightSidebar bind:this={rightSidebarEl} />"
content = content.replace(old3, new3)

print("ref:", "rightSidebarEl" in content)
print("shortcut:", "switchToAi" in content)
print("bind:", "bind:this={rightSidebarEl}" in content)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print("done")
