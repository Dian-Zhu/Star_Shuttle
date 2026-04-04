#!/usr/bin/env python3
path = '/home/rust/Star_Shuttle/src-tauri/src/lib.rs'
content = open(path, 'r', encoding='utf-8').read()

# 1. Add AgentManager after ChatManager initialization
old1 = """            // Initialize AI ChatManager
            let chat_manager = Arc::new(crate::modules::ai::chat::ChatManager::new(db.clone()));
            app.manage(chat_manager);"""

new1 = """            // Initialize AI ChatManager
            let chat_manager = Arc::new(crate::modules::ai::chat::ChatManager::new(db.clone()));
            app.manage(chat_manager);

            // Initialize AI AgentManager
            let agent_manager = Arc::new(crate::modules::ai::agent::AgentManager::new(
                db.clone(),
                manager.clone(),
            ));
            app.manage(agent_manager);"""

content = content.replace(old1, new1)

# 2. Add Agent commands to invoke_handler
old2 = """            crate::modules::ai::ai_get_terminal_context,
        ])"""
new2 = """            crate::modules::ai::ai_get_terminal_context,
            // Agent commands
            crate::modules::ai::ai_agent_start,
            crate::modules::ai::ai_agent_confirm,
            crate::modules::ai::ai_agent_cancel,
            crate::modules::ai::ai_agent_status,
        ])"""
content = content.replace(old2, new2)

print("AgentManager:", "AgentManager::new" in content)
print("agent_start:", "ai_agent_start" in content)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print("done")
