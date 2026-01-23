//! 记忆提取器测试程序
//! 
//! 运行方式: 
//!   cargo run --bin test_extractor -- "对话文件路径"
//!   cargo run --bin test_extractor  (使用内置测试对话)

use memo_chater::config::AppConfig;
use memo_chater::extractor::{MemoryExtractor, ExtractorConfig};
use memo_chater::types::ChatMessage;
use std::env;
use std::fs;

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== 记忆提取器测试 ===\n");

    // 获取当前日期
    let today = chrono::Local::now().format("%Y年%m月%d日").to_string();
    println!("当前日期: {}\n", today);

    // 从config.toml读取配置
    let app_config = AppConfig::load_default().expect("加载配置失败");
    
    let config = ExtractorConfig {
        api_base: app_config.ai.api_base.clone(),
        api_key: app_config.ai.get_api_key().unwrap_or_default(),
        model: app_config.ai.extractor_model.clone(),
        custom_prompt: None,
        user_name: app_config.roles.user_name.clone(),
        assistant_name: app_config.roles.assistant_name.clone(),
    };

    println!("配置 (从config.toml读取):");
    println!("  API Base: {}", config.api_base);
    println!("  Model: {}", config.model);
    println!("  API Key: {}...", &config.api_key.chars().take(10).collect::<String>());
    println!();

    let extractor = MemoryExtractor::new(config);

    // 检查命令行参数
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        // 从文件读取对话
        let file_path = &args[1];
        println!("从文件读取对话: {}\n", file_path);
        
        match fs::read_to_string(file_path) {
            Ok(content) => {
                let messages = parse_markdown_conversation(&content);
                if messages.is_empty() {
                    println!("警告: 未能从文件中解析出对话内容");
                    return;
                }
                println!("解析到 {} 条消息\n", messages.len());
                run_test(&extractor, &messages, &today).await;
            }
            Err(e) => {
                println!("读取文件失败: {}", e);
                return;
            }
        }
    } else {
        // 使用内置测试对话
        println!("未指定文件，使用内置测试对话\n");
        println!("用法: cargo run --bin test_extractor -- \"对话文件路径\"\n");
        run_builtin_tests(&extractor, &today).await;
    }

    println!("\n=== 测试完成 ===");
}

/// 解析 Markdown 格式的对话
fn parse_markdown_conversation(content: &str) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    let mut current_role: Option<String> = None;
    let mut current_content = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        
        // 检测角色标记
        if trimmed.starts_with("## ") {
            // 保存之前的消息
            if let Some(role) = current_role.take() {
                let content = current_content.trim().to_string();
                if !content.is_empty() {
                    messages.push(ChatMessage {
                        role,
                        content,
                    });
                }
                current_content.clear();
            }
            
            // 解析新角色
            let role_part = &trimmed[3..];
            if role_part.contains("User") || role_part.contains("秦") || role_part.contains("🧑") {
                current_role = Some("user".to_string());
            } else if role_part.contains("Assistant") || role_part.contains("诺亚") || role_part.contains("🤖") {
                current_role = Some("assistant".to_string());
            }
        } else if current_role.is_some() && !trimmed.starts_with("---") && !trimmed.starts_with("# ") {
            // 累积内容
            if !current_content.is_empty() {
                current_content.push('\n');
            }
            current_content.push_str(line);
        }
    }

    // 保存最后一条消息
    if let Some(role) = current_role {
        let content = current_content.trim().to_string();
        if !content.is_empty() {
            messages.push(ChatMessage {
                role,
                content,
            });
        }
    }

    messages
}

async fn run_builtin_tests(extractor: &MemoryExtractor, today: &str) {
    println!("--- 测试: 基础信息提取 ---");
    let messages = vec![
        ChatMessage::user("你好，我叫秦，是一名Rust程序员"),
        ChatMessage::assistant("你好秦！很高兴认识你。"),
        ChatMessage::user("主要做后端开发，最近在研究AI记忆系统"),
    ];
    run_test(extractor, &messages, today).await;
}

async fn run_test(extractor: &MemoryExtractor, messages: &[ChatMessage], _today: &str) {
    println!("输入对话 ({} 条消息):", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        let preview: String = msg.content.chars().take(100).collect();
        let suffix = if msg.content.chars().count() > 100 { "..." } else { "" };
        println!("  {}. [{}]: {}{}", i + 1, msg.role, preview, suffix);
    }
    println!();

    println!("正在调用AI提取记忆...\n");
    match extractor.extract(messages).await {
        Ok(result) => {
            println!("========== 提取结果 ==========");
            println!("解析成功: {}", result.parse_success);
            println!("提取到 {} 条记忆:\n", result.memories.len());
            
            for (i, memory) in result.memories.iter().enumerate() {
                println!("【记忆 {}】", i + 1);
                println!("  {}", memory.content);
                if let Some(t) = &memory.memory_type {
                    println!("  类型: {}", t);
                }
                if let Some(imp) = memory.importance {
                    println!("  重要性: {}", imp);
                }
                if !memory.entities.is_empty() {
                    println!("  实体: {:?}", memory.entities);
                }
                println!();
            }
            
            if !result.warnings.is_empty() {
                println!("警告: {:?}\n", result.warnings);
            }
            
            println!("========== 原始AI响应 ==========");
            println!("{}", result.raw_response);
            println!("================================");
        }
        Err(e) => {
            println!("提取失败: {}", e);
        }
    }
}